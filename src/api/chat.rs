use axum::{
    extract::Json,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
};
use futures::stream::Stream;
use tokio_stream::StreamExt;
use std::convert::Infallible;

use crate::api::{err_json, ok_json, ApiResponse};
use crate::chat::BackendFlavor;
use crate::config;

pub async fn handle_chat_send(
    Json(body): Json<serde_json::Value>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiResponse> {
    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    if message.is_empty() {
        return Err(err_json("Message is empty"));
    }

    let message = message.to_string();
    let rx = tokio::task::spawn_blocking(move || {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.send_message_stream(&message)
            .map_err(|e| config::keys::redact_keys(&e))
    }).await.unwrap().map_err(|e| err_json(&e))?;

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(|data| Ok(Event::default().data(data)));

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

pub async fn handle_chat_new_session() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.new_session()?;
        Ok::<_, String>(())
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_chat_switch_model(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return err_json("Missing 'name' field"),
    };

    let result = tokio::task::spawn_blocking(move || {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        let model_name = service.switch_model_rpc(&name)?;
        Ok::<_, String>(model_name)
    }).await.unwrap();

    match result {
        Ok(model_name) => ok_json(serde_json::json!({"ok": true, "model": model_name})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_chat_backend_status() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        Ok::<_, String>(service.current_flavor().as_str().to_string())
    }).await.unwrap();

    match result {
        Ok(backend) => ok_json(serde_json::json!({"ok": true, "data": {"backend": backend}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_chat_switch_backend(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let raw = match body.get("backend").and_then(|v| v.as_str()) {
        Some(s) => s.trim().to_string(),
        None => return err_json("Missing 'backend' field"),
    };

    let normalized = raw.to_ascii_lowercase();
    if !matches!(normalized.as_str(), "pi" | "little-coder" | "littlecoder") {
        return err_json("Invalid backend (use 'pi' or 'little-coder')");
    }
    let flavor = BackendFlavor::from_str(&raw);

    let result = tokio::task::spawn_blocking(move || {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.switch_backend_flavor(flavor)?;

        // Persistir la elección para las próximas sesiones (default_flavor la lee
        // como prioridad 1 al arrancar). Un fallo de escritura no invalida el switch.
        if let Err(e) = crate::config::props::UserProps::set("agent_backend", flavor.as_str()) {
            eprintln!("[chat] Failed to persist agent_backend prop: {}", e);
        }

        Ok::<_, String>(())
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true, "data": {"backend": flavor.as_str()}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}
