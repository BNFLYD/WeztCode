use axum::{
    extract::Json,
    response::sse::{Event, KeepAlive, Sse},
    response::IntoResponse,
};
use futures::stream::Stream;
use tokio_stream::StreamExt;
use std::convert::Infallible;

use crate::api::{err_json, ok_json, ApiResponse};
use crate::chat;
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
        let models = crate::config::models::list();
        let entry = models.into_iter()
            .find(|m| m.name == name)
            .ok_or_else(|| format!("Model '{}' not found", name))?;

        let config = chat::ChatConfig::from_model_entry(&entry);
        let new_backend: Box<dyn chat::AgentBackend> = Box::new(chat::PiAgentBackend::new(config));

        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.switch_backend(new_backend)?;
        Ok::<_, String>(entry.name)
    }).await.unwrap();

    match result {
        Ok(model_name) => ok_json(serde_json::json!({"ok": true, "model": model_name})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}
