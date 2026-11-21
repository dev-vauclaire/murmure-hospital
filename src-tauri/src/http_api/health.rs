use super::types::{HealthResponse, TranscribeState};
use crate::audio::types::AudioState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use tauri::Manager;

pub(super) async fn health_handler(State(state): State<TranscribeState>) -> Response {
    let audio_state = state.app.state::<AudioState>();
    health_response(&audio_state)
}

fn health_response(audio_state: &AudioState) -> Response {
    // Reading readiness must not wait for inference or trigger model loading.
    let (http_status, status) = if audio_state.is_engine_ready() {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not_ready")
    };

    (
        http_status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(HealthResponse {
            service: "murmure Vauclaire",
            status,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::health_response;
    use crate::audio::types::AudioState;
    use axum::{body::to_bytes, http::StatusCode, response::Response};
    use serde_json::{json, Value};
    use std::{sync::Arc, time::Duration};

    async fn response_json(response: Response, expected_status: StatusCode) -> Value {
        assert_eq!(response.status(), expected_status);
        assert_eq!(response.headers()["content-type"], "application/json");
        assert_eq!(response.headers()["cache-control"], "no-store");
        let body = to_bytes(response.into_body(), 16_384).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn unloaded_engine_returns_not_ready_without_starting_work() {
        let audio_state = AudioState::new();

        let body = response_json(
            health_response(&audio_state),
            StatusCode::SERVICE_UNAVAILABLE,
        )
        .await;

        assert_eq!(
            body,
            json!({"service": "murmure Vauclaire", "status": "not_ready"})
        );
        assert!(!audio_state.is_engine_ready());
        assert!(audio_state.engine.lock().is_none());
        assert!(audio_state.recorder.lock().is_none());
        assert!(!audio_state.is_session_active());
        assert_eq!(audio_state.current_session(), 0);
    }

    #[tokio::test]
    async fn loaded_engine_returns_ready_even_with_an_active_session() {
        let audio_state = AudioState::new();
        audio_state.mark_engine_ready();
        let session = audio_state.begin_session();
        audio_state.set_session_active(true);

        let body = response_json(health_response(&audio_state), StatusCode::OK).await;

        assert_eq!(
            body,
            json!({"service": "murmure Vauclaire", "status": "ready"})
        );
        assert!(audio_state.is_engine_ready());
        assert!(audio_state.is_session_active());
        assert_eq!(audio_state.current_session(), session);
    }

    #[tokio::test]
    async fn health_response_does_not_wait_for_the_engine_mutex() {
        let audio_state = Arc::new(AudioState::new());
        audio_state.mark_engine_ready();
        let engine_guard = audio_state.engine.lock();
        let state = audio_state.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || tx.send(health_response(&state)).unwrap());

        let result = rx.recv_timeout(Duration::from_secs(1));
        // Release the guard before joining, including if a future regression blocks the handler.
        drop(engine_guard);
        worker.join().unwrap();

        let response = result.expect("health must respond without acquiring the engine mutex");
        let body = response_json(response, StatusCode::OK).await;
        assert_eq!(body["status"], "ready");
    }
}
