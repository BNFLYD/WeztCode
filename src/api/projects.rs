use crate::api::{json_error, json_response, read_request_body};

pub fn handle_projects_list() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let projects = crate::config::project_dirs::list();
    json_response(&serde_json::json!({ "ok": true, "data": projects }))
}

pub fn handle_projects_add(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let path = parsed.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let response = match crate::config::project_dirs::add(path) {
        Ok(projects) => json_response(&serde_json::json!({ "ok": true, "data": projects })),
        Err(e) => json_error(&e),
    };
    let _ = request.respond(response);
}

pub fn handle_projects_delete(path: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::project_dirs::remove(path) {
        Ok(projects) => json_response(&serde_json::json!({ "ok": true, "data": projects })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_projects_switch(path: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let dir = std::path::Path::new(path);
    if !dir.is_dir() {
        return json_error("Directory not found");
    }
    match crate::config::props::UserProps::set("current_dir", path) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}
