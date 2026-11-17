use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn open_user_guide(app: AppHandle) -> Result<(), String> {
    // Resolve only the bundled guide; the frontend cannot supply another path.
    let path =
        crate::utils::resources::resolve_resource_path(&app, "hospital/user-guide/index.html")
            .filter(|path| path.is_file())
            .ok_or_else(|| "The bundled user guide is unavailable.".to_string())?;
    let path = path
        .to_str()
        .ok_or_else(|| "The user guide path is not supported.".to_string())?;

    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|_| "Unable to open the user guide.".to_string())
}
