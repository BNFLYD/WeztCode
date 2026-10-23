use axum::{
    extract::{Json, Query},
    http::{header, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
};
use futures::stream::Stream;
use tokio_stream::StreamExt;
use std::collections::HashMap;
use std::convert::Infallible;

use crate::api::{err_json, ok_json, ApiResponse};
use crate::chat::BackendFlavor;

/// Stream SSE desde un receiver de broadcast del turno: termina al ver el
/// evento done (el sender vive en el ActiveTurn del servicio, así que el
/// Closed del broadcast no llega nunca mientras el turno exista).
fn turn_sse_stream(
    mut rx: tokio::sync::broadcast::Receiver<String>,
) -> impl futures::Stream<Item = Result<Event, Infallible>> {
    futures::stream::unfold((rx, false), |(mut rx, done_seen)| async move {
        if done_seen {
            return None;
        }
        loop {
            match rx.recv().await {
                Ok(sse) => {
                    let is_done = sse.contains("\"type\":\"done\"");
                    let item = Ok(Event::default().data(sse));
                    if is_done {
                        return Some((item, (rx, true)));
                    }
                    return Some((item, (rx, false)));
                }
                // Consumidor lento: salta los eventos lagged — el buffer del
                // ActiveTurn persiste todo para un futuro re-attach.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => return None,
            }
        }
    })
}
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

    Ok(Sse::new(turn_sse_stream(rx)).keep_alive(KeepAlive::default()))
}

/// GET /chat/stream — re-attach a un turno en curso (la UI se monta tras un
/// cambio de sección): replay del buffer del turno + eventos live hasta done.
/// Si no hay turno activo, emite un único evento `idle` y cierra — la UI
/// reconcilia con el historial y listo.
pub async fn handle_chat_stream(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, ApiResponse> {
    // ?agent=X: el turno de ESE scope (default: el activo). La UI re-attacha
    // al scope que está viendo — con multi-agente, el turno de otro agente
    // puede seguir corriendo en paralelo.
    let agent = params.get("agent").cloned();
    let attach = tokio::task::spawn_blocking(move || {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.attach_turn(agent.as_deref())
    }).await.unwrap().map_err(|e| err_json(&e))?;

    type ChatStream = std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, Infallible>> + Send>>;
    match attach {
        // Sin turno en curso (o ya terminado: el reconcile lo trae del historial)
        None => {
            let idle = Event::default().data("{\"type\":\"idle\"}");
            Ok(Sse::new(
                Box::pin(futures::stream::iter(vec![Ok::<_, Infallible>(idle)])) as ChatStream,
            ))
        }
        Some((agent, snapshot, live_rx)) => {
            // Primer evento: "attached" — la UI lo usa para decidir ANTES de
            // reconciliar (con turno activo el reconcile se salta: el RPC
            // compite por el stdout que consume el reader del stream).
            let attached = serde_json::json!({"type": "attached", "agent": agent})
                .to_string();
            // Replay del buffer (todo lo que pasó mientras la UI estuvo fuera)
            // encadenado con los eventos live a partir de acá.
            let replay = futures::stream::iter(
                std::iter::once(Ok::<_, Infallible>(Event::default().data(attached)))
                    .chain(
                        snapshot
                            .into_iter()
                            .map(|sse| Ok::<_, Infallible>(Event::default().data(sse))),
                    )
                    .collect::<Vec<_>>(),
            );
            let live = turn_sse_stream(live_rx);
            let chained = futures::stream::StreamExt::chain(replay, live);
            Ok(Sse::new(Box::pin(chained) as ChatStream))
        }
    }
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

/// GET /chat/stats — stats REALES de la sesión activa: contextUsage calculado
/// por pi contra la ventana real del modelo en uso, tokens y costo. Es el
/// mismo número que muestra la TUI de pi.
/// GET /chat/conversations — lista las conversaciones del proyecto actual
/// con la sesión actual del scope activo marcada (picker de la UI).
pub async fn handle_chat_conversations() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.list_conversations()
    }).await.unwrap();

    match result {
        Ok(json) => {
            let v: serde_json::Value =
                serde_json::from_str(&json).unwrap_or(serde_json::json!({}));
            ok_json(serde_json::json!({"ok": true, "data": v}))
        }
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// POST /chat/delete-conversation — borra el archivo de una conversación del
/// proyecto actual. Body: {"id": "<session-id>"}. La conversación activa no
/// se puede borrar (el proceso pi la usa).
pub async fn handle_chat_delete_conversation(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let Some(id) = body.get("id").and_then(|v| v.as_str()) else {
        return err_json("Missing 'id' field (session-id string)");
    };
    let id = id.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.delete_conversation(&id)
    }).await.unwrap();

    match result {
        Ok(()) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// POST /chat/select-conversation — cambia el scope activo a una conversación
/// existente del proyecto actual. Body: {"id": "<session-id>"}.
pub async fn handle_chat_select_conversation(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let Some(session_id) = body.get("id").and_then(|v| v.as_str()) else {
        return err_json("Missing 'id' field (session-id string)");
    };
    let session_id = session_id.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let mut service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.select_conversation(&session_id)
    }).await.unwrap();

    match result {
        Ok(v) => ok_json(serde_json::json!({"ok": true, "data": v})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_chat_stats() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.get_session_stats()
    }).await.unwrap();

    match result {
        Ok(json) => {
            let v: serde_json::Value =
                serde_json::from_str(&json).unwrap_or(serde_json::json!({}));
            let data = v.get("data").cloned().unwrap_or(serde_json::json!({}));
            ok_json(serde_json::json!({"ok": true, "data": data}))
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
        Ok(agent) => ok_json(serde_json::json!({"ok": true, "data": {
            "agent": if agent.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(agent) },
            // Proyecto actual: el frontend key-ea su cache de mensajes por
            // proyecto+agente (el instant-paint tras un switch nunca muestra
            // la conversación de otro proyecto).
            "project": crate::config::current_root::get().to_string_lossy(),
        }})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

/// GET /chat/asset?agent=Miku&state=standby|talking|thinking|confused — imagen
/// del estado visual de un agente (channel del chat en el monitor). El agente va
/// EXPLÍCITO en el URL para que el cache del browser key-ee por agent+state
/// (switch de agente → URL distinto → imagen correcta). Éxito: cache fuerte de
/// un día; error: 404 + no-store (una respuesta JSON rota no debe pegarse).
pub async fn handle_chat_asset(
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let agent = params.get("agent").cloned().unwrap_or_default();
    let state = params.get("state").cloned().unwrap_or_default();
    let log_ctx = format!("agent={} state={}", agent, state);

    let result = tokio::task::spawn_blocking(move || {
        let service = crate::CHAT_SERVICE.lock()
            .map_err(|e| format!("Lock: {}", e))?;
        service.agent_asset(&agent, &state)
    }).await.unwrap();

    match result {
        Ok((content_type, bytes)) => {
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type.as_str()),
                    (header::CACHE_CONTROL, "public, max-age=86400"),
                ],
                bytes,
            ).into_response()
        }
        Err(e) => {
            eprintln!("[chat] asset error ({}): {}", log_ctx, e);
            (
                StatusCode::NOT_FOUND,
                [
                    (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                config::keys::redact_keys(&e),
            ).into_response()
        }
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
