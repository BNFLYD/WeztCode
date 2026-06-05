use crate::api::{json_error, json_response, parse_query_param, read_request_body};
use crate::terminal::{TerminalProtocol, WeztermProtocol};

pub fn handle_terminal_list() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let term = WeztermProtocol::new();
    match term.list_panes_structured() {
        Ok(panes) => {
            let metadata = crate::config::terms_metadata::list();
            let data = serde_json::json!({
                "ok": true,
                "data": {
                    "panes": panes,
                    "metadata": metadata
                }
            });
            json_response(&data)
        }
        Err(e) => json_error(&e),
    }
}

pub fn handle_terminal_spawn(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let name = parsed
        .get("name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let icon = parsed
        .get("icon")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let program = parsed
        .get("program")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let cwd = crate::config::props::UserProps::load()
        .get("current_dir")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let term = WeztermProtocol::new();
    let response = match term.spawn_tab(cwd.as_deref(), program.as_deref()) {
        Ok(pane_id) => {
            if name.is_some() || icon.is_some() {
                let _ = crate::config::terms_metadata::set(pane_id, name.clone(), icon.clone());
            }
            let data = serde_json::json!({
                "ok": true,
                "data": {
                    "pane_id": pane_id,
                    "name": name,
                    "icon": icon
                }
            });
            json_response(&data)
        }
        Err(e) => json_error(&e),
    };
    let _ = request.respond(response);
}

pub fn handle_terminal_kill(pane_id: u32) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if pane_id == 0 {
        return json_error("Invalid pane_id");
    }
    let term = WeztermProtocol::new();
    match term.kill_pane(pane_id) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_terminal_activate(pane_id: u32) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if pane_id == 0 {
        return json_error("Invalid pane_id");
    }
    let term = WeztermProtocol::new();
    match term.activate_pane(pane_id) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_terminal_metadata(mut request: tiny_http::Request) {
    let url = request.url().to_string();

    if url.starts_with("/api/terminal/metadata/set") {
        let body = read_request_body(&mut request);
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let pane_id = parsed
            .get("pane_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let name = parsed
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let icon = parsed
            .get("icon")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let response = match crate::config::terms_metadata::set(pane_id, name, icon) {
            Ok(_) => json_response(&serde_json::json!({ "ok": true })),
            Err(e) => json_error(&e),
        };
        let _ = request.respond(response);
        return;
    }

    if url.starts_with("/api/terminal/metadata/delete") {
        let pane_id = parse_query_param(&url, "pane_id")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let response = match crate::config::terms_metadata::remove(pane_id) {
            Ok(_) => json_response(&serde_json::json!({ "ok": true })),
            Err(e) => json_error(&e),
        };
        let _ = request.respond(response);
        return;
    }

    let metadata = crate::config::terms_metadata::list();
    let response = json_response(&serde_json::json!({ "ok": true, "data": metadata }));
    let _ = request.respond(response);
}

pub fn handle_terminal_edit_defaults(request: tiny_http::Request) {
    let path = crate::config::default_terms::detect_path_str();
    let response = match std::process::Command::new("nvim")
        .args(["--server", "/tmp/weztcode-nvim.sock", "--remote", &path])
        .output()
    {
        Ok(o) if o.status.success() => json_response(&serde_json::json!({ "ok": true })),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            json_error(&format!("nvim failed: {}", stderr))
        }
        Err(e) => json_error(&format!("Failed to run nvim: {}", e)),
    };
    let _ = request.respond(response);
}

pub fn handle_terminal_default_terms(request: tiny_http::Request) {
    let terms = crate::config::default_terms::list();
    let response = json_response(&serde_json::json!({ "ok": true, "data": terms }));
    let _ = request.respond(response);
}

pub fn handle_active_pane() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let path = match std::env::var("WEZTCODE_ACTIVE_PANE_FILE") {
        Ok(p) => p,
        Err(_) => return json_error("ACTIVE_PANE_FILE not set"),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let pane_id = content.trim().parse::<u32>().unwrap_or(0);
            let data = serde_json::json!({
                "ok": true,
                "data": { "pane_id": pane_id }
            });
            json_response(&data)
        }
        Err(e) => json_error(&format!("Failed to read active_pane: {}", e)),
    }
}

pub fn handle_terminal_ensure_main() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let term = WeztermProtocol::new();
    match term.activate_pane(0) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}
