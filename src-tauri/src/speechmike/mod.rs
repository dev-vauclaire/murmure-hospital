#[cfg(any(target_os = "windows", test))]
mod control;
mod profiles;
#[cfg(any(target_os = "windows", test))]
mod protocol;
#[cfg(any(target_os = "windows", test))]
mod types;

pub(crate) use profiles::MicrophoneControls;
#[cfg(target_os = "windows")]
pub(crate) use profiles::{is_keyboard_control_action, KeyboardAdmission};

#[cfg(target_os = "windows")]
mod identity_windows;
#[cfg(target_os = "windows")]
mod runtime;
#[cfg(target_os = "windows")]
mod windows;

pub(crate) fn init(_app: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    runtime::init(_app);
}

pub(crate) fn shutdown(_app: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    runtime::shutdown(_app);
}

pub(crate) fn request_shutdown(_app: &tauri::AppHandle, _code: Option<i32>) -> bool {
    #[cfg(target_os = "windows")]
    {
        runtime::request_shutdown(_app, _code)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub(crate) fn status(_app: &tauri::AppHandle) -> &'static str {
    #[cfg(target_os = "windows")]
    {
        runtime::status(_app)
    }
    #[cfg(not(target_os = "windows"))]
    {
        "unsupported"
    }
}

pub(crate) fn microphone_controls(_app: &tauri::AppHandle) -> MicrophoneControls {
    #[cfg(target_os = "windows")]
    {
        runtime::microphone_controls(_app)
    }
    #[cfg(not(target_os = "windows"))]
    {
        MicrophoneControls::default()
    }
}

pub(crate) fn request_context_refresh(_app: &tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    runtime::request_context_refresh(_app);
}

pub(crate) fn with_microphone_selection_change<T>(
    _app: &tauri::AppHandle,
    change: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    #[cfg(target_os = "windows")]
    {
        runtime::with_microphone_selection_change(_app, change)
    }
    #[cfg(not(target_os = "windows"))]
    {
        change()
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn keyboard_state(app: &tauri::AppHandle) -> (bool, u64) {
    runtime::keyboard_state(app)
}

#[cfg(target_os = "windows")]
pub(crate) fn handle_keyboard_event(
    app: &tauri::AppHandle,
    action: &crate::shortcuts::types::ShortcutAction,
    mode: &crate::shortcuts::types::ActivationMode,
    event: crate::shortcuts::types::KeyEventType,
) {
    runtime::handle_keyboard_event(app, action, mode, event);
}

/// Runs independently of Tauri so the diagnostic never opens audio or user stores.
pub(crate) fn run_diagnostic() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        windows::run_diagnostic()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Le diagnostic SpeechMike est disponible uniquement sous Windows.".to_string())
    }
}
