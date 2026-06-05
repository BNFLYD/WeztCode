pub mod chat;
pub mod editor;
pub mod fs;
pub mod keys;
pub mod models;
pub mod projects;
pub mod terminal;

use std::io::Read;
use std::path::{Path, PathBuf};

use tiny_http;

pub fn dispatch(mut request: tiny_http::Request, url: &str) {
    if url == "/api/chat/send" {
        return chat::handle_chat_send(request);
    }

    if url.starts_with("/api/keys/set") {
        return keys::handle_keys_set(request);
    }

    if url.starts_with("/api/projects/add") {
        return projects::handle_projects_add(request);
    }

    if url.starts_with("/api/terminal/spawn") {
        return terminal::handle_terminal_spawn(request);
    }

    if url.starts_with("/api/terminal/metadata") {
        return terminal::handle_terminal_metadata(request);
    }

    if url.starts_with("/api/terminal/default-terms") {
        return terminal::handle_terminal_default_terms(request);
    }

    if url.starts_with("/api/terminal/edit-defaults") {
        return terminal::handle_terminal_edit_defaults(request);
    }

    if url == "/api/chat/new-session" {
        return chat::handle_chat_new_session(request);
    }

    if url == "/api/chat/switch-model" {
        return chat::handle_chat_switch_model(request);
    }

    if url.starts_with("/api/models/list") {
        return models::handle_models_list(request);
    }

    if url.starts_with("/api/models/edit-defaults") {
        return models::handle_models_edit_defaults(request);
    }

    let root = crate::config::props::UserProps::load()
        .get("current_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let response: tiny_http::Response<std::io::Cursor<Vec<u8>>> =
        if url.starts_with("/api/terminal/list") {
            terminal::handle_terminal_list()
        } else if url.starts_with("/api/terminal/kill") {
            let pane_id = parse_query_param(url, "pane_id")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            terminal::handle_terminal_kill(pane_id)
        } else if url.starts_with("/api/terminal/active-pane") {
            terminal::handle_active_pane()
        } else if url.starts_with("/api/terminal/ensure-main") {
            terminal::handle_terminal_ensure_main()
        } else if url.starts_with("/api/terminal/activate") {
            let pane_id = parse_query_param(url, "pane_id")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            terminal::handle_terminal_activate(pane_id)
        } else if url.starts_with("/api/projects/list") {
            projects::handle_projects_list()
        } else if url.starts_with("/api/projects/delete") {
            let path = parse_query_param(url, "path").unwrap_or_default();
            projects::handle_projects_delete(&path)
        } else if url.starts_with("/api/projects/switch") {
            let path = parse_query_param(url, "path").unwrap_or_default();
            projects::handle_projects_switch(&path)
        } else if url.starts_with("/api/keys/delete") {
            let name = parse_query_param(url, "name").unwrap_or_default();
            keys::handle_keys_delete(&name)
        } else if url.starts_with("/api/keys/list") {
            keys::handle_keys_list()
        } else if url.starts_with("/api/fs/ls") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| "/".to_string());
            fs::handle_ls(&rel_path, &root)
        } else if url.starts_with("/api/fs/read") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            fs::handle_read(&rel_path, &root)
        } else if url.starts_with("/api/editor/open") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            editor::handle_editor_open(&rel_path, &root)
        } else if url.starts_with("/api/fs/create") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            fs::handle_create(&rel_path, &root)
        } else if url.starts_with("/api/fs/delete") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            fs::handle_delete(&rel_path, &root)
        } else if url.starts_with("/api/fs/rename") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            let new_name = parse_query_param(url, "name").unwrap_or_else(|| String::new());
            fs::handle_rename(&rel_path, &new_name, &root)
        } else if url.starts_with("/api/fs/move") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            let dest = parse_query_param(url, "dest").unwrap_or_else(|| String::new());
            fs::handle_move(&rel_path, &dest, &root)
        } else if url.starts_with("/api/fs/image") {
            let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
            fs::handle_image(rel_path, &root)
        } else {
            json_error("Unknown API endpoint")
        };

    let _ = request.respond(response);
}

pub fn json_response(
    data: &serde_json::Value,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_string(data).unwrap_or_default();
    tiny_http::Response::from_string(body).with_header(tiny_http::Header {
        field: "Content-Type".parse().unwrap(),
        value: "application/json".parse().unwrap(),
    })
}

pub fn json_error(msg: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let data = serde_json::json!({ "ok": false, "error": msg });
    json_response(&data)
}

pub fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            let raw = parts.next().unwrap_or("");
            return Some(url_decode(raw));
        }
    }
    None
}

pub fn url_decode(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            bytes.extend_from_slice(encoded.as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn read_request_body(request: &mut tiny_http::Request) -> String {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    body
}

pub fn is_keys_path(path: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_default();
    let keys_path = crate::config::keys::KeysStore::load_path();
    canonical == keys_path.canonicalize().unwrap_or_default()
}
