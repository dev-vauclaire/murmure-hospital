use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::shortcuts::types::{ActivationMode, KeyEventType, ShortcutAction, ShortcutState};

use super::types::{Button, ButtonUpdate, ControlAction, Controls};
use super::windows::{self, Event, Status};

struct RuntimeState {
    stop: Arc<AtomicBool>,
    status: Mutex<&'static str>,
    worker: Mutex<Option<JoinHandle<()>>>,
    shutting_down: AtomicBool,
    shutdown_complete: AtomicBool,
}

pub(super) fn init(app: &AppHandle) {
    let stop = Arc::new(AtomicBool::new(false));
    if !app.manage(RuntimeState {
        stop: stop.clone(),
        status: Mutex::new("searching"),
        worker: Mutex::new(None),
        shutting_down: AtomicBool::new(false),
        shutdown_complete: AtomicBool::new(false),
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
                    let shortcut_state = worker_app.state::<ShortcutState>();
                    let enabled = ready
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

fn update_status(app: &AppHandle, next: &'static str) {
    if let Some(state) = app.try_state::<RuntimeState>() {
        let mut current = state.status.lock();
        if *current == next {
            return;
        }
        *current = next;
        drop(current);
        let _ = app.emit("speechmike-status", next);
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
    match action {
        ControlAction::Record(mode, event) => {
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
