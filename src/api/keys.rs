use crate::api::{json_error, json_response, read_request_body};
use crate::config::keys::KeysStore;

pub fn handle_keys_set(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let _ = request.respond(json_error("Invalid JSON body"));
            return;
        }
    };
    let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let value = parsed.get("value").and_then(|v| v.as_str()).unwrap_or("");
    if name.is_empty() {
        let _ = request.respond(json_error("Missing 'name' field"));
        return;
    }
    match KeysStore::set(name, value) {
        Ok(_) => {
            let _ = request.respond(json_response(&serde_json::json!({ "ok": true })));
        }
        Err(e) => {
            let _ = request.respond(json_error(&e));
        }
    }
}

pub fn handle_keys_delete(name: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if name.is_empty() {
        return json_error("Missing 'name' query param");
    }
    match KeysStore::delete(name) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_keys_list() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let store = KeysStore::load();
    let names = store.list_names();
    json_response(&serde_json::json!({ "ok": true, "keys": names }))
}
