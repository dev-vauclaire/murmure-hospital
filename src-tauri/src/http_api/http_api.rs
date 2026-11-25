use log::error;
use std::io::ErrorKind;
use std::sync::atomic::Ordering;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::http_api::types::{HttpApiServerState, HttpApiStartError, HttpApiState};

pub fn spawn_http_api_thread(
    app_handle: AppHandle,
    port: u16,
    state: HttpApiState,
) -> Result<(), HttpApiStartError> {
    if state
        .is_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    let (listener, runtime) = match prepare_server(port) {
        Ok(prepared) => prepared,
        Err(start_error) => {
            mark_not_running(
                &state,
                start_error.server_state(),
                port,
                Some(start_error.to_string()),
            );
            return Err(start_error);
        }
    };

    state.set_status(HttpApiServerState::Running, port, None);

    std::thread::spawn(move || {
        match runtime.block_on(crate::http_api::server::start_http_api(
            app_handle,
            listener,
            state.clone(),
        )) {
            Ok(()) => mark_not_running(&state, HttpApiServerState::Stopped, port, None),
            Err(e) => {
                error!("HTTP API error: {}", e);
                mark_not_running(
                    &state,
                    HttpApiServerState::Failed,
                    port,
                    Some(e.to_string()),
                );
            }
        }
    });

    Ok(())
}

pub fn notify_start_error_dialog(app_handle: &AppHandle, error: &HttpApiStartError) {
    let msg = match error {
        HttpApiStartError::PortInUse(port) => format!(
            "Impossible d'activer le partage Murmure : le port {} est déjà utilisé par une autre application.\n\nFermez cette application, puis réessayez depuis « Partage Murmure » dans les paramètres Système.",
            port
        ),
        HttpApiStartError::Other(_) => {
            "Impossible d'activer le partage Murmure sur cet ordinateur.\n\nRéessayez depuis « Partage Murmure » dans les paramètres Système.".to_string()
        }
    };
    let app_handle = app_handle.clone();
    std::thread::spawn(move || {
        let _ = app_handle
            .dialog()
            .message(&msg)
            .title("Partage Murmure")
            .kind(tauri_plugin_dialog::MessageDialogKind::Error)
            .blocking_show();
    });
}

fn prepare_server(
    port: u16,
) -> Result<(std::net::TcpListener, tokio::runtime::Runtime), HttpApiStartError> {
    let listener = bind_listener(port).map_err(|e| match e.kind() {
        ErrorKind::AddrInUse => HttpApiStartError::PortInUse(port),
        _ => HttpApiStartError::Other(e.to_string()),
    })?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| {
        HttpApiStartError::Other(format!(
            "Failed to create async runtime for HTTP API: {}",
            e
        ))
    })?;
    Ok((listener, runtime))
}

fn bind_listener(port: u16) -> std::io::Result<std::net::TcpListener> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

fn mark_not_running(
    state: &HttpApiState,
    server_state: HttpApiServerState,
    port: u16,
    message: Option<String>,
) {
    state.set_status(server_state, port, message);
    state.is_running.store(false, Ordering::SeqCst);
}
