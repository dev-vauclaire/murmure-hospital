use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::shortcuts::types::{ActivationMode, KeyEventType, ShortcutAction, ShortcutState};

use super::profiles::{
    is_keyboard_control_action, IdentityStatus, InputProfile, MicrophoneControls, ProfileController,
};
use super::types::{Button, ButtonUpdate, ControlAction, Controls};
use super::windows::{self, Event, Status};

struct RuntimeState {
    stop: Arc<AtomicBool>,
    status: Mutex<&'static str>,
    worker: Mutex<Option<JoinHandle<()>>>,
    shutting_down: AtomicBool,
    shutdown_complete: AtomicBool,
    profile: Mutex<ProfileController>,
    refresh_requested: AtomicBool,
    selection_revision: AtomicU64,
}

/// A selection revision is odd while its settings/cache are being replaced.
/// Drop restores a completed revision even if native device lookup fails.
struct SelectionChange<'a>(&'a RuntimeState);

impl Drop for SelectionChange<'_> {
    fn drop(&mut self) {
        self.0.selection_revision.fetch_add(1, Ordering::SeqCst);
        self.0.refresh_requested.store(true, Ordering::SeqCst);
    }
}

pub(super) fn init(app: &AppHandle) {
    let stop = Arc::new(AtomicBool::new(false));
    if !app.manage(RuntimeState {
        stop: stop.clone(),
        status: Mutex::new("searching"),
        worker: Mutex::new(None),
        shutting_down: AtomicBool::new(false),
        shutdown_complete: AtomicBool::new(false),
        profile: Mutex::new(ProfileController::default()),
        refresh_requested: AtomicBool::new(true),
        selection_revision: AtomicU64::new(0),
    }) {
        return;
    }
    let worker_app = app.clone();
    let result = thread::Builder::new()
        .name("speechmike".to_string())
        .spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                let mut controls = Controls::default();
                let mut ready = false;
                let mut next_identity_check = Instant::now();
                let mut profile_revision = 0;
                let result = windows::run_service(stop.clone(), |event| {
                    if let Event::Status(device_status) = &event {
                        ready = *device_status == Status::Connected;
                        controls.reset_baseline();
                        update_status(
                            &worker_app,
                            if ready {
                                "initializing"
                            } else {
                                status_code(*device_status)
                            },
                        );
                    }
                    if let Event::Buttons(update) = &event {
                        if ready && controls.observe_baseline(update) {
                            update_status(&worker_app, "ready");
                        }
                    }
                    refresh_context(&worker_app, &mut next_identity_check);
                    let (profile, revision) = active_profile(&worker_app);
                    if revision != profile_revision {
                        controls.rearm();
                        profile_revision = revision;
                        if ready && !controls.baseline_ready {
                            update_status(&worker_app, "initializing");
                        }
                    }
                    let shortcut_state = worker_app.state::<ShortcutState>();
                    let enabled = ready
                        && profile == InputProfile::SpeechMike
                        && controls.baseline_ready
                        && !stop.load(Ordering::SeqCst)
                        && !shortcut_state.is_suspended()
                        && !shortcut_state.is_capturing();
                    if let Some(action) = controls.set_enabled(enabled) {
                        dispatch(&worker_app, action);
                    }
                    controls.set_record_available(
                        enabled
                            && crate::shortcuts::shortcuts::speechmike_input_available(&worker_app),
                    );
                    if let Event::Buttons(update) = event {
                        let mode = if record_was_pressed(&update) && enabled {
                            match crate::settings::load_settings(&worker_app)
                                .record_mode
                                .as_str()
                            {
                                "toggle_to_talk" => ActivationMode::ToggleToTalk,
                                _ => ActivationMode::PushToTalk,
                            }
                        } else {
                            // A release uses the mode captured at press, not this fallback.
                            ActivationMode::PushToTalk
                        };
                        for action in controls.buttons(update, mode) {
                            dispatch(&worker_app, action);
                        }
                    }
                });
                crate::shortcuts::shortcuts::cancel_speechmike_recording(&worker_app);
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if result.is_err() {
                    update_status(&worker_app, "unavailable");
                }
                // Retry an unavailable HID stack without delaying application shutdown.
                for _ in 0..50 {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    refresh_context(&worker_app, &mut next_identity_check);
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
    match result {
        Ok(worker) => *app.state::<RuntimeState>().worker.lock() = Some(worker),
        Err(_) => update_status(app, "unavailable"),
    }
}

pub(super) fn shutdown(app: &AppHandle) {
    if let Some(state) = app.try_state::<RuntimeState>() {
        state.stop.store(true, Ordering::SeqCst);
        // Exit can no longer service tray/UI tasks. Never join a worker here.
        // A normal quit has already joined it off-thread in request_shutdown.
        drop(state.worker.lock().take());
    }
}

/// Keep the event loop alive while the HID worker finishes audio/UI cleanup.
pub(super) fn request_shutdown(app: &AppHandle, code: Option<i32>) -> bool {
    // Tauri explicitly ignores prevent_exit for a process restart.
    if code == Some(tauri::RESTART_EXIT_CODE) {
        return false;
    }
    let Some(state) = app.try_state::<RuntimeState>() else {
        return false;
    };
    if state.shutdown_complete.load(Ordering::SeqCst) {
        return false;
    }
    if state.shutting_down.swap(true, Ordering::SeqCst) {
        return true;
    }
    app.state::<ShortcutState>().set_suspended(true);
    state.stop.store(true, Ordering::SeqCst);
    let app_for_shutdown = app.clone();
    let result = thread::Builder::new()
        .name("speechmike-shutdown".to_string())
        .spawn(move || {
            if let Some(state) = app_for_shutdown.try_state::<RuntimeState>() {
                if let Some(worker) = state.worker.lock().take() {
                    // A failing USB/audio driver must not prevent quitting forever.
                    let until = std::time::Instant::now() + Duration::from_secs(5);
                    while !worker.is_finished() && std::time::Instant::now() < until {
                        thread::sleep(Duration::from_millis(100));
                    }
                    if worker.is_finished() {
                        let _ = worker.join();
                    }
                }
                state.shutdown_complete.store(true, Ordering::SeqCst);
            }
            app_for_shutdown.exit(code.unwrap_or(0));
        });
    if result.is_err() {
        state.shutdown_complete.store(true, Ordering::SeqCst);
        return false;
    }
    true
}

pub(super) fn status(app: &AppHandle) -> &'static str {
    app.try_state::<RuntimeState>()
        .map(|state| *state.status.lock())
        .unwrap_or("unavailable")
}

pub(super) fn microphone_controls(app: &AppHandle) -> MicrophoneControls {
    app.try_state::<RuntimeState>()
        .map(|state| state.profile.lock().snapshot())
        .unwrap_or(MicrophoneControls {
            speech_mike_status: "unavailable",
            identity_status: "unavailable",
            ..MicrophoneControls::default()
        })
}

pub(super) fn request_context_refresh(app: &AppHandle) {
    if let Some(state) = app.try_state::<RuntimeState>() {
        state.refresh_requested.store(true, Ordering::SeqCst);
    }
}

/// Called off the main thread. The same profile lock serializes selection writes
/// and native input admission; the lifecycle barrier protects queued audio starts.
pub(super) fn with_microphone_selection_change<T>(
    app: &AppHandle,
    change: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let Some(state) = app.try_state::<RuntimeState>() else {
        return change();
    };
    let _profile = state.profile.lock();
    let lifecycle = crate::shortcuts::types::recording_state()
        .lifecycle
        .try_lock()
        .ok_or_else(|| {
            "L'audio est en cours d'initialisation. Réessayez dans un instant.".to_string()
        })?;
    if lifecycle.has_pending_start() {
        return Err("La dictée est en cours de démarrage. Réessayez dans un instant.".to_string());
    }
    state.selection_revision.fetch_add(1, Ordering::SeqCst);
    state.refresh_requested.store(true, Ordering::SeqCst);
    let _selection_change = SelectionChange(&state);
    change()
}

fn active_profile(app: &AppHandle) -> (InputProfile, u64) {
    app.try_state::<RuntimeState>()
        .map(|state| {
            let profile = state.profile.lock();
            (profile.profile(), profile.revision())
        })
        .unwrap_or((InputProfile::Keyboard, 0))
}

pub(super) fn keyboard_state(app: &AppHandle) -> (bool, u64) {
    let (profile, revision) = active_profile(app);
    (profile == InputProfile::Keyboard, revision)
}

/// Serialize admission with a profile transition. Shared native actions remain
/// callable by HID/CLI; only the physical keyboard enters through this guard.
pub(super) fn handle_keyboard_event(
    app: &AppHandle,
    action: &ShortcutAction,
    mode: &ActivationMode,
    event: KeyEventType,
) {
    if !is_keyboard_control_action(action) {
        crate::shortcuts::handle_shortcut_event(app, action, mode, event);
        return;
    }
    let Some(state) = app.try_state::<RuntimeState>() else {
        crate::shortcuts::handle_shortcut_event(app, action, mode, event);
        return;
    };
    let mut profile = state.profile.lock();
    if profile.profile() != InputProfile::Keyboard {
        return;
    }
    if *action == ShortcutAction::StartRecording
        && !profile.admit_record_event(
            InputProfile::Keyboard,
            event,
            state.selection_revision.load(Ordering::SeqCst),
            crate::shortcuts::shortcuts::is_recording_idle(),
            recording_busy(app),
        )
    {
        return;
    }
    crate::shortcuts::handle_shortcut_event(app, action, mode, event);
}

fn recording_busy(app: &AppHandle) -> bool {
    !crate::shortcuts::shortcuts::is_recording_idle()
        || crate::shortcuts::types::recording_state()
            .ending
            .load(Ordering::SeqCst)
        || app
            .state::<crate::audio::types::AudioState>()
            .is_session_active()
}

fn refresh_context(app: &AppHandle, next_check: &mut Instant) {
    let Some(state) = app.try_state::<RuntimeState>() else {
        return;
    };
    let now = Instant::now();
    let refresh = state.refresh_requested.swap(false, Ordering::SeqCst);
    let identity = if refresh || now >= *next_check {
        *next_check = now + Duration::from_secs(1);
        let selection_revision = state.selection_revision.load(Ordering::SeqCst);
        if !selection_revision.is_multiple_of(2) {
            None
        } else {
            let settings = crate::settings::load_settings(app);
            let status = status(app);
            let observed = if matches!(status, "multiple_devices" | "unsupported_device") {
                IdentityStatus::NotMatched
            } else {
                match super::identity_windows::selected_input_matches_speechmike(
                    settings.mic_id.as_deref(),
                ) {
                    Ok(true) => IdentityStatus::Matched,
                    Ok(false) => IdentityStatus::NotMatched,
                    Err(()) => IdentityStatus::Unavailable,
                }
            };
            Some((observed, selection_revision))
        }
    } else {
        None
    };

    let (previous, next) = {
        let mut profile = state.profile.lock();
        let previous = profile.snapshot();
        if let Some((identity, selection_revision)) = identity {
            // Recheck after acquiring the admission lock, not just after COM:
            // a setter may have changed the choice while this thread waited.
            if state.selection_revision.load(Ordering::SeqCst) == selection_revision {
                profile.confirm_identity(identity, selection_revision);
            } else {
                state.refresh_requested.store(true, Ordering::SeqCst);
            }
        }
        profile.settle(recording_busy(app));
        (previous, profile.snapshot())
    };
    if previous != next {
        let _ = app.emit("microphone-controls-changed", next);
    }
}

fn update_status(app: &AppHandle, next: &'static str) {
    if let Some(state) = app.try_state::<RuntimeState>() {
        let mut current = state.status.lock();
        if *current == next {
            return;
        }
        *current = next;
        drop(current);
        let _ = app.emit("speechmike-status", next);
        let (previous, snapshot) = {
            let mut profile = state.profile.lock();
            let previous = profile.snapshot();
            profile.set_status(next);
            if matches!(
                next,
                "disconnected" | "multiple_devices" | "unsupported_device"
            ) {
                profile.set_identity(IdentityStatus::NotMatched);
                state.refresh_requested.store(true, Ordering::SeqCst);
            }
            profile.settle(recording_busy(app));
            (previous, profile.snapshot())
        };
        if previous != snapshot {
            let _ = app.emit("microphone-controls-changed", snapshot);
        }
    }
}

fn status_code(status: Status) -> &'static str {
    match status {
        Status::Waiting => "searching",
        Status::Connected => "ready",
        Status::Disconnected => "disconnected",
        Status::MultipleCandidates => "multiple_devices",
        Status::DifferentMode(_) => "wrong_mode",
        Status::DifferentModel(_) => "unsupported_device",
        Status::EnumerationFailed
        | Status::OpenFailed
        | Status::IdentificationFailed
        | Status::ModeReadFailed => "unavailable",
    }
}

fn record_was_pressed(update: &ButtonUpdate) -> bool {
    match update {
        ButtonUpdate::InitialState(_) => false,
        ButtonUpdate::Changes(changes) => changes
            .iter()
            .any(|change| change.button == Button::Record && change.pressed),
    }
}

fn dispatch(app: &AppHandle, action: ControlAction) {
    // Cancellation on disconnect must still reach the owned session after a
    // profile change. All other hardware actions belong to the hardware profile.
    if action == ControlAction::CancelOwned {
        crate::shortcuts::shortcuts::cancel_speechmike_recording(app);
        return;
    }
    let Some(state) = app.try_state::<RuntimeState>() else {
        return;
    };
    let mut profile = state.profile.lock();
    if profile.profile() != InputProfile::SpeechMike {
        return;
    }
    match action {
        ControlAction::Record(mode, event) => {
            if !profile.admit_record_event(
                InputProfile::SpeechMike,
                event,
                state.selection_revision.load(Ordering::SeqCst),
                crate::shortcuts::shortcuts::is_recording_idle(),
                recording_busy(app),
            ) {
                return;
            }
            crate::shortcuts::shortcuts::handle_speechmike_record_event(app, &mode, event);
        }
        ControlAction::CancelOwned => {
            crate::shortcuts::shortcuts::cancel_speechmike_recording(app);
        }
        ControlAction::Cancel | ControlAction::PasteLast => {
            let shortcut_state = app.state::<ShortcutState>();
            if shortcut_state.is_suspended() || shortcut_state.is_capturing() {
                return;
            }
            let action = match action {
                ControlAction::Cancel => ShortcutAction::CancelRecording,
                _ => ShortcutAction::PasteLastTranscript,
            };
            crate::shortcuts::handle_shortcut_event(
                app,
                &action,
                &ActivationMode::PushToTalk,
                KeyEventType::Pressed,
            );
        }
    }
}
