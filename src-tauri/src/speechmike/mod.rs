#[cfg(any(target_os = "windows", test))]
mod control;
#[cfg(any(target_os = "windows", test))]
mod protocol;
#[cfg(any(target_os = "windows", test))]
mod types;

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
