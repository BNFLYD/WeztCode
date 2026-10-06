use axum::body::Bytes;
use axum::response::IntoResponse;

use crate::api::{err_json, ok_json};
use crate::config;

const MAX_WAV_BYTES: usize = 10 * 1024 * 1024; // 10 MB (~5 min de WAV 16k mono)

pub async fn handle_transcribe(body: Bytes) -> impl IntoResponse {
    if body.is_empty() {
        return err_json("Body vacío (se espera audio/wav)");
    }
    if body.len() > MAX_WAV_BYTES {
        return err_json("Audio demasiado grande (máx 10MB)");
    }

    let result = tokio::task::spawn_blocking(move || crate::stt::transcribe(&body))
        .await
        .unwrap();

    match result {
        Ok(text) => ok_json(serde_json::json!({"ok": true, "data": {"text": text}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_native_start() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| crate::stt_native::native_start())
        .await
        .unwrap();
    match result {
        Ok(()) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_native_stop() -> impl IntoResponse {
    let result =
        tokio::task::spawn_blocking(|| crate::stt_native::native_stop_and_transcribe())
            .await
            .unwrap();
    match result {
        Ok(text) => ok_json(serde_json::json!({"ok": true, "data": {"text": text}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_native_cancel() -> impl IntoResponse {
    crate::stt_native::native_cancel();
    ok_json(serde_json::json!({"ok": true}))
}
