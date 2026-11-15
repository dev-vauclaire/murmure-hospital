use tauri::{command, AppHandle};

#[command]
pub fn get_speechmike_status(app: AppHandle) -> &'static str {
    crate::speechmike::status(&app)
}

// Admission may briefly wait for the native recording controller. Keep this
// snapshot request off the main UI thread while a tray operation is pending.
#[command]
pub async fn get_microphone_controls(app: AppHandle) -> crate::speechmike::MicrophoneControls {
    crate::speechmike::microphone_controls(&app)
}
