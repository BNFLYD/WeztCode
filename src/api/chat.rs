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
        service.new_conversation()?;
        Ok::<_, String>(())
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// POST /chat/select-scope — selector unificado: cambia al scope de un agente
/// (por nombre) o al chat default (agent: null). Cada scope tiene su propia
/// sesión persistida; el backend retoma/crea la sesión correspondiente.
pub async fn handle_chat_select_scope(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let agent = match body.get("agent") {
        Some(serde_json::Value::Null) => None,
        Some(v) => match v.as_str() {
            Some(s) => Some(s.to_string()),
            None => return err_json("Field 'agent' must be a string or null"),
        },
        None => return err_json("Missing 'agent' field (string or null)"),
    };

    let result = tokio::task::spawn_blocking(move || {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        let outcome = service.select_scope(agent.as_deref())?;
        Ok::<_, String>(serde_json::json!({
            "agent": agent,
            "backend": outcome.backend.as_str(),
            "warning": outcome.warning,
        }))
    }).await.unwrap();

    match result {
        Ok(data) => ok_json(serde_json::json!({"ok": true, "data": data})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// GET /chat/messages — historial REAL de la sesión activa (fuente de verdad
/// para que la UI reconcilie su cache al cambiar de scope).
pub async fn handle_chat_messages() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.get_messages()
    }).await.unwrap();

    match result {
        Ok(json) => {
            let v: serde_json::Value =
                serde_json::from_str(&json).unwrap_or(serde_json::json!({}));
            let messages = v
                .get("data")
                .and_then(|d| d.get("messages"))
                .cloned()
                .unwrap_or(serde_json::json!([]));
            ok_json(serde_json::json!({"ok": true, "data": {"messages": messages}}))
        }
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// GET /chat/active-agent — scope con el que booteó el backend (para que el
/// frontend arranque mostrando la conversación correcta).
pub async fn handle_chat_active_agent() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        Ok::<_, String>(service.active_agent().map(str::to_string).unwrap_or_default())
    }).await.unwrap();

    match result {
        Ok(agent) => ok_json(serde_json::json!({"ok": true, "data": {"agent": if agent.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(agent) }}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// POST /chat/abort — aborta el turno EN CURSO del agente (pi/little-coder).
/// A diferencia del corte de conexión del frontend, esto le dice al proceso
/// que deje de pensar/ejecutar de verdad.
pub async fn handle_chat_abort() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.abort()?;
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
        Ok::<_, String>(())
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true, "data": {"backend": flavor.as_str()}})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}
