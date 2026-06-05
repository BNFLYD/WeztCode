use crate::api::{json_error, json_response};

pub fn handle_models_list(request: tiny_http::Request) {
    let models = crate::config::models::list();
    let response = json_response(&serde_json::json!({ "ok": true, "data": models }));
    let _ = request.respond(response);
}

pub fn handle_models_edit_defaults(request: tiny_http::Request) {
    let path = crate::config::models::detect_path_str();
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
