use axum::response::IntoResponse;

use crate::api::{err_json, ok_json};

pub async fn handle_models_list() -> impl IntoResponse {
    let models = crate::config::models::list();
    ok_json(serde_json::json!({"ok": true, "data": models}))
}

pub async fn handle_models_edit_defaults() -> impl IntoResponse {
    let path = crate::config::models::detect_path_str();
    let result = tokio::task::spawn_blocking(move || {
        std::process::Command::new("nvim")
            .args(["--server", "/tmp/weztcode-nvim.sock", "--remote", &path])
            .output()
    }).await.unwrap();

    match result {
        Ok(o) if o.status.success() => ok_json(serde_json::json!({"ok": true})),
        Ok(o) => err_json(&format!("nvim failed: {}", String::from_utf8_lossy(&o.stderr))),
        Err(e) => err_json(&format!("Failed to run nvim: {}", e)),
    }
}
