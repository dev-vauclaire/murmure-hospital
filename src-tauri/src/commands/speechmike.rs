use tauri::{command, AppHandle};

#[command]
pub fn get_speechmike_status(app: AppHandle) -> &'static str {
    crate::speechmike::status(&app)
}
