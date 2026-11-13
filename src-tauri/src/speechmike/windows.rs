use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use super::protocol::{self, ButtonTracker, ButtonUpdate};

const DIAGNOSTIC_DURATION: Duration = Duration::from_secs(60);
const SCAN_INTERVAL: Duration = Duration::from_secs(1);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);
const READ_POLL: Duration = Duration::from_millis(100);
const QUERY_TIMEOUT: Duration = Duration::from_secs(1);
const REPORT_BUFFER_SIZE: usize = 256;
const MAX_UNKNOWN_REPORT_NOTICES: u8 = 3;

struct Connection {
    path: CString,
    device: HidDevice,
    buttons: ButtonTracker,
    pending_baseline: Option<u16>,
    unknown_reports: u8,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct InterfaceMetadata {
    vendor_id: u16,
    product_id: u16,
    usage_page: u16,
    usage: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
    Waiting,
    MultipleCandidates,
    EnumerationFailed,
    OpenFailed,
    IdentificationFailed,
    DifferentModel(u16),
    ModeReadFailed,
    DifferentMode(u8),
    Connected,
    Disconnected,
}

pub(super) enum Event {
    Status(Status),
    Buttons(ButtonUpdate),
    Tick,
}

pub(super) fn run_diagnostic() -> Result<(), String> {
    println!("Diagnostic SpeechMike SMP3700 : 60 secondes, Ctrl+C pour arrêter.");
    println!("Aucun audio, aucune transcription et aucune modification du périphérique.");
    println!(
        "Filtre du SDK de référence : VID={:04x}, PID={:04x}, usage_page={:04x}, usage={:04x}.",
        protocol::VENDOR_ID,
        protocol::PRODUCT_ID,
        protocol::USAGE_PAGE,
        protocol::USAGE
    );
    println!("Utiliser le mode Event et relâcher les boutons avant de commencer.");

    let confirmed_device = watch(
        &AtomicBool::new(false),
        Some(Instant::now() + DIAGNOSTIC_DURATION),
        &mut print_diagnostic_event,
        true,
    )?;
    if !confirmed_device {
        println!("Aucun SMP3700 n'a été confirmé en mode Event pendant ce diagnostic.");
    }
    println!("Fin du diagnostic. Aucun enregistrement audio n'a été déclenché.");
    Ok(())
}

pub(super) fn run_service(
    stop: Arc<AtomicBool>,
    mut on_event: impl FnMut(Event),
) -> Result<(), String> {
    watch(&stop, None, &mut on_event, false).map(|_| ())
}

fn watch(
    stop: &AtomicBool,
    deadline: Option<Instant>,
    on_event: &mut impl FnMut(Event),
    diagnostic: bool,
) -> Result<bool, String> {
    if stopped(stop, deadline) {
        return Ok(false);
    }
    let mut api =
        HidApi::new().map_err(|_| "Impossible d'initialiser l'accès HID Windows.".to_string())?;
    let mut next_scan = Instant::now();
    let mut retry_at = Instant::now();
    let mut connection: Option<Connection> = None;
    let mut status = None;
    let mut confirmed_device = false;
    let mut previous_interfaces = None;

    while !stopped(stop, deadline) {
        on_event(Event::Tick);
        if stopped(stop, deadline) {
            break;
        }
        let now = Instant::now();
        if now >= next_scan {
            next_scan = now + SCAN_INTERVAL;
            match api.refresh_devices() {
                Ok(()) => {
                    if diagnostic {
                        show_observed_interfaces(&api, &mut previous_interfaces);
                    }
                    let paths = candidate_paths(&api);
                    let disconnected = connection
                        .as_ref()
                        .is_some_and(|active| !paths.contains(&active.path));
                    if disconnected {
                        connection = None;
                        retry_at = now;
                        notify(&mut status, Status::Disconnected, on_event);
                    }

                    match paths.as_slice() {
                        [] => {
                            connection = None;
                            notify(&mut status, Status::Waiting, on_event);
                        }
                        [path] if connection.is_none() && now >= retry_at => {
                            retry_at = now + RETRY_INTERVAL;
                            match connect(&api, path, stop, deadline) {
                                Ok(active) => {
                                    connection = Some(active);
                                    confirmed_device = true;
                                    notify(&mut status, Status::Connected, on_event);
                                }
                                Err(problem) if !stopped(stop, deadline) => {
                                    notify(&mut status, problem, on_event)
                                }
                                Err(_) => {}
                            }
                        }
                        [_] if connection.is_some() => {
                            let recovering = status != Some(Status::Connected);
                            notify(&mut status, Status::Connected, on_event);
                            if recovering && !stopped(stop, deadline) {
                                // A transient enumeration failure suspends control.
                                // Re-establish the current baseline before new edges.
                                if let Some(snapshot) = connection
                                    .as_ref()
                                    .and_then(|active| active.buttons.snapshot())
                                {
                                    on_event(Event::Buttons(snapshot));
                                }
                            }
                        }
                        [_] => {}
                        _ => {
                            // Picking an arbitrary device would make a button test ambiguous.
                            connection = None;
                            notify(&mut status, Status::MultipleCandidates, on_event);
                        }
                    }
                }
                Err(_) => notify(&mut status, Status::EnumerationFailed, on_event),
            }
        }

        if stopped(stop, deadline) {
            break;
        }
        let read_failed = match connection.as_mut() {
            Some(active) => match read_buttons(active, stop, deadline, diagnostic) {
                Ok(Some(update)) if !stopped(stop, deadline) => {
                    on_event(Event::Buttons(update));
                    false
                }
                Ok(_) => false,
                Err(()) => true,
            },
            None => {
                thread::sleep(wait_duration(deadline));
                false
            }
        };
        if read_failed {
            connection = None;
            retry_at = Instant::now() + RETRY_INTERVAL;
            next_scan = Instant::now();
            notify(&mut status, Status::Disconnected, on_event);
        }
    }

    if !diagnostic && connection.is_some() {
        notify(&mut status, Status::Disconnected, on_event);
    }
    drop(connection);
    Ok(confirmed_device)
}

fn show_observed_interfaces(api: &HidApi, previous: &mut Option<Vec<InterfaceMetadata>>) {
    let mut interfaces: Vec<_> = api
        .device_list()
        .filter(|device| device.vendor_id() == protocol::VENDOR_ID)
        .map(|device| InterfaceMetadata {
            vendor_id: device.vendor_id(),
            product_id: device.product_id(),
            usage_page: device.usage_page(),
            usage: device.usage(),
        })
        .collect();
    interfaces.sort_unstable();
    interfaces.dedup();
    if previous.as_ref() == Some(&interfaces) {
        return;
    }
    if interfaces.is_empty() {
        println!("Aucune interface HID Philips observée.");
    } else {
        println!("Interfaces HID Philips observées (métadonnées uniquement) :");
        for interface in &interfaces {
            println!(
                "  VID={:04x}, PID={:04x}, usage_page={:04x}, usage={:04x}",
                interface.vendor_id, interface.product_id, interface.usage_page, interface.usage
            );
        }
    }
    *previous = Some(interfaces);
}

fn candidate_paths(api: &HidApi) -> Vec<CString> {
    let mut paths: Vec<_> = api
        .device_list()
        .filter(|device| {
            device.vendor_id() == protocol::VENDOR_ID
                && device.product_id() == protocol::PRODUCT_ID
                && device.usage_page() == protocol::USAGE_PAGE
                && device.usage() == protocol::USAGE
        })
        .map(|device| device.path().to_owned())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

fn connect(
    api: &HidApi,
    path: &CStr,
    stop: &AtomicBool,
    deadline: Option<Instant>,
) -> Result<Connection, Status> {
    if stopped(stop, deadline) {
        return Err(Status::OpenFailed);
    }
    let device = api.open_path(path).map_err(|_| Status::OpenFailed)?;
    let mut baseline = None;
    let identity = query(
        &device,
        protocol::DEVICE_CODE_COMMAND,
        stop,
        deadline,
        &mut baseline,
    )
    .map_err(|()| Status::IdentificationFailed)?;
    let code = protocol::device_code(&identity).map_err(|_| Status::IdentificationFailed)?;
    if code != 3700 {
        return Err(Status::DifferentModel(code));
    }

    let reply = query(
        &device,
        protocol::EVENT_MODE_COMMAND,
        stop,
        deadline,
        &mut baseline,
    )
    .map_err(|()| Status::ModeReadFailed)?;
    let mode = protocol::event_mode(&reply).map_err(|_| Status::ModeReadFailed)?;
    if mode != 0 {
        return Err(Status::DifferentMode(mode));
    }

    Ok(Connection {
        path: path.to_owned(),
        device,
        buttons: ButtonTracker::default(),
        pending_baseline: baseline,
        unknown_reports: 0,
    })
}

fn query(
    device: &HidDevice,
    command: u8,
    stop: &AtomicBool,
    deadline: Option<Instant>,
    baseline: &mut Option<u16>,
) -> Result<Vec<u8>, ()> {
    let query_deadline = Instant::now() + QUERY_TIMEOUT;
    let until = Some(deadline.map_or(query_deadline, |deadline| deadline.min(query_deadline)));
    if stopped(stop, until) {
        return Err(());
    }

    // HIDAPI requires the zero Report ID for writes, but strips it from reads.
    // These are read-only identification queries, never SET commands.
    // The Windows backend can return Ok(0) for a successful synchronous write.
    device.write(&[0, command]).map_err(|_| ())?;
    let mut buffer = [0; REPORT_BUFFER_SIZE];
    while !stopped(stop, until) {
        let size = device
            .read_timeout(&mut buffer, timeout_ms(until))
            .map_err(|_| ())?;
        if size > 0 {
            let report = &buffer[..size];
            if report[0] == command {
                return Ok(report.to_vec());
            }
            if report[0] == protocol::BUTTONS_COMMAND {
                // Identification and button reports share the input queue. Keep
                // the last observed state, but never replay pre-ready presses.
                // An invalid update may hide a release or a press, so an older
                // state is no longer a safe baseline in that case.
                *baseline = protocol::button_mask(report).ok();
            }
        }
    }
    Err(())
}

fn read_buttons(
    active: &mut Connection,
    stop: &AtomicBool,
    deadline: Option<Instant>,
    diagnostic: bool,
) -> Result<Option<ButtonUpdate>, ()> {
    if stopped(stop, deadline) {
        return Ok(None);
    }
    if let Some(mask) = active.pending_baseline.take() {
        // Connected has already been emitted. Publish only the final observed
        // handshake state as a baseline, not its intermediate transitions.
        return Ok(Some(active.buttons.update(mask)));
    }
    let mut buffer = [0; REPORT_BUFFER_SIZE];
    match active
        .device
        .read_timeout(&mut buffer, timeout_ms(deadline))
    {
        Ok(0) => Ok(None),
        Ok(size) => {
            let report = &buffer[..size];
            match protocol::button_mask(report) {
                Ok(mask) => {
                    let update = active.buttons.update(mask);
                    if matches!(&update, ButtonUpdate::Changes(changes) if changes.is_empty()) {
                        Ok(None)
                    } else {
                        Ok(Some(update))
                    }
                }
                Err(_) => {
                    // A malformed button update could hide a release. Runtime
                    // control must drop the connection so its session is cancelled.
                    if !diagnostic && report[0] == protocol::BUTTONS_COMMAND {
                        return Err(());
                    }
                    if diagnostic && active.unknown_reports < MAX_UNKNOWN_REPORT_NOTICES {
                        println!(
                            "Rapport non interprété : code=0x{:02x}, longueur={}. Aucun contenu affiché.",
                            report[0], size
                        );
                        active.unknown_reports += 1;
                    }
                    Ok(None)
                }
            }
        }
        Err(_) => Err(()),
    }
}

fn stopped(stop: &AtomicBool, deadline: Option<Instant>) -> bool {
    stop.load(Ordering::Acquire) || deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

fn wait_duration(deadline: Option<Instant>) -> Duration {
    deadline.map_or(READ_POLL, |deadline| {
        deadline
            .saturating_duration_since(Instant::now())
            .min(READ_POLL)
    })
}

fn timeout_ms(deadline: Option<Instant>) -> i32 {
    wait_duration(deadline).as_millis().max(1) as i32
}

fn notify(previous: &mut Option<Status>, next: Status, on_event: &mut impl FnMut(Event)) {
    if *previous == Some(next) {
        return;
    }
    *previous = Some(next);
    on_event(Event::Status(next));
}

fn print_diagnostic_event(event: Event) {
    match event {
        Event::Tick => {}
        Event::Buttons(ButtonUpdate::InitialState(buttons)) => {
            let labels: Vec<_> = buttons.into_iter().map(|button| button.label()).collect();
            if labels.is_empty() {
                println!("État initial : aucun bouton appuyé.");
            } else {
                println!(
                    "État initial, boutons déjà appuyés : {}.",
                    labels.join(", ")
                );
            }
        }
        Event::Buttons(ButtonUpdate::Changes(changes)) => {
            for change in changes {
                println!(
                    "{} {}",
                    change.button.label(),
                    if change.pressed {
                        "appuyé"
                    } else {
                        "relâché"
                    }
                );
            }
        }
        Event::Status(status) => print_diagnostic_status(status),
    }
}

fn print_diagnostic_status(status: Status) {
    match status {
        Status::Waiting => println!("En attente d'une interface SpeechMike compatible..."),
        Status::MultipleCandidates => println!(
            "Plusieurs interfaces candidates détectées : conserver un seul SpeechMike connecté. Lecture suspendue."
        ),
        Status::EnumerationFailed => println!("Énumération HID indisponible ; nouvel essai dans une seconde."),
        Status::OpenFailed => println!(
            "Interface candidate inaccessible. Vérifier les applications utilisant le micro ; nouvel essai dans cinq secondes."
        ),
        Status::IdentificationFailed => println!(
            "Interface candidate trouvée, mais réponse d'identification absente ou incompatible ; nouvel essai dans cinq secondes."
        ),
        Status::DifferentModel(code) => println!(
            "Modèle déclaré par le périphérique : {}. Ce diagnostic attend un SMP3700 ; lecture des boutons désactivée.", code
        ),
        Status::ModeReadFailed => println!(
            "SMP3700 identifié, mais son mode n'a pas pu être lu ; nouvel essai dans cinq secondes."
        ),
        Status::DifferentMode(mode) => println!(
            "SMP3700 identifié en mode {} : sélectionner le mode Event avant de poursuivre. Le diagnostic ne change pas le mode.", mode
        ),
        Status::Connected => println!(
            "SMP3700 identifié et connecté en mode Event. Appuyer puis relâcher REC et les autres boutons."
        ),
        Status::Disconnected => println!("SpeechMike déconnecté ou lecture interrompue ; état des boutons réinitialisé."),
    }
}
