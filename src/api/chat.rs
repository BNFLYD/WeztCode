use crate::api::{json_error, json_response, read_request_body};
use crate::chat;
use crate::config;

pub fn handle_chat_send(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);

    let message = match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(val) => val
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        Err(_) => {
            let err = json_error("Invalid JSON body");
            let _ = request.respond(err);
            return;
        }
    };

    if message.is_empty() {
        let err = json_error("Message is empty");
        let _ = request.respond(err);
        return;
    }

    let reader = {
        let mut service = match crate::CHAT_SERVICE.lock() {
            Ok(s) => s,
            Err(_) => {
                let err = json_error("Chat service lock failed");
                let _ = request.respond(err);
                return;
            }
        };

        match service.send_message_stream(&message) {
            Ok(r) => r,
            Err(e) => {
                let safe = config::keys::redact_keys(&format!("Chat error: {}", e));
                let err = json_error(&safe);
                let _ = request.respond(err);
                return;
            }
        }
    };

    let response = tiny_http::Response::new(
        tiny_http::StatusCode(200),
        vec![
            tiny_http::Header {
                field: "Content-Type".parse().unwrap(),
                value: "text/event-stream".parse().unwrap(),
            },
            tiny_http::Header {
                field: "Cache-Control".parse().unwrap(),
                value: "no-cache".parse().unwrap(),
            },
            tiny_http::Header {
                field: "Access-Control-Allow-Origin".parse().unwrap(),
                value: "*".parse().unwrap(),
            },
        ],
        reader,
        None,
        None,
    );

    let _ = request.respond(response);

    if let Ok(service) = crate::CHAT_SERVICE.lock() {
        match service.get_session_stats() {
            Ok(stats) => eprintln!("[pi] session stats: {}", stats),
            Err(e) => eprintln!("[pi] session stats error: {}", e),
        }
        match service.get_state() {
            Ok(state) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&state) {
                    let level = json
                        .get("data")
                        .and_then(|d| d.get("thinkingLevel"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("(not found)");
                    eprintln!("[pi] thinking level: {}", level);
                }
            }
            Err(e) => eprintln!("[pi] get_state error: {}", e),
        }
    }
}

pub fn handle_chat_new_session(mut request: tiny_http::Request) {
    let mut service = match crate::CHAT_SERVICE.lock() {
        Ok(s) => s,
        Err(_) => {
            let _ = request.respond(json_error("Chat service lock failed"));
            return;
        }
    };

    match service.new_session() {
        Ok(_) => {
            let _ = request.respond(json_response(&serde_json::json!({ "ok": true })));
        }
        Err(e) => {
            let safe = config::keys::redact_keys(&format!("Failed to start new session: {}", e));
            let _ = request.respond(json_error(&safe));
        }
    }
}

pub fn handle_chat_switch_model(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let _ = request.respond(json_error("Invalid JSON body"));
            return;
        }
    };

    let name = match parsed.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => {
            let _ = request.respond(json_error("Missing 'name' field"));
            return;
        }
    };

    let models = crate::config::models::list();
    let entry = match models.into_iter().find(|m| m.name == name) {
        Some(e) => e,
        None => {
            let _ = request.respond(json_error(&format!("Model '{}' not found", name)));
            return;
        }
    };

    let config = chat::ChatConfig::from_model_entry(&entry);
    let new_backend: Box<dyn chat::AgentBackend> = Box::new(chat::PiAgentBackend::new(config));

    let mut service = match crate::CHAT_SERVICE.lock() {
        Ok(s) => s,
        Err(_) => {
            let _ = request.respond(json_error("Chat service lock failed"));
            return;
        }
    };

    match service.switch_backend(new_backend) {
        Ok(_) => {
            let _ = request.respond(json_response(&serde_json::json!({
                "ok": true,
                "model": entry.name
            })));
        }
        Err(e) => {
            let safe = config::keys::redact_keys(&format!("Failed to switch model: {}", e));
            let _ = request.respond(json_error(&safe));
        }
    }
}
