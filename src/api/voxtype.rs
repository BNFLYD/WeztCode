//! Endpoints del STT por voxtype (mic del chat).
//!
//! - POST /stt/voxtype/start  → inicia la grabación
//! - POST /stt/voxtype/stop   → detiene, espera la transcripción y devuelve el texto
//! - GET  /stt/voxtype/status → estado del daemon ("idle", "stopped", ...)

use axum::response::IntoResponse;

use crate::api::{err_json, ok_json};
use crate::config;

/// POST /stt/voxtype/start — inicia la grabación por mic (daemon voxtype).
pub async fn handle_voxtype_start() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(crate::stt_voxtype::record_start)
        .await
        .unwrap();
    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// POST /stt/voxtype/stop — detiene, espera la transcripción (event-driven,
/// sin polling) y devuelve el texto dictado.
pub async fn handle_voxtype_stop() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(crate::stt_voxtype::record_stop_and_wait)
        .await
        .unwrap();
    match result {
        Ok(text) => ok_json(serde_json::json!({"ok": true, "data": {"text": text}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// GET /stt/voxtype/status — estado del daemon.
pub async fn handle_voxtype_status() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(crate::stt_voxtype::status)
        .await
        .unwrap();
    match result {
        Ok(status) => ok_json(serde_json::json!({"ok": true, "data": {"status": status}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}
