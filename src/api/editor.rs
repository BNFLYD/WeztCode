use std::collections::HashMap;

use axum::extract::Query;
use axum::response::IntoResponse;

use crate::api::{err_json, get_current_root};

pub async fn handle_editor_open(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let rel_path = params.get("path").map(|s| s.to_string()).unwrap_or_default();

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        let full_path = crate::config::fs::sanitize_path(&rel_path, &root)
            .map_err(|e| format!("{}", e))?;

        if !full_path.is_file() {
            return Err("Not a file".to_string());
        }

        let output = std::process::Command::new("nvim")
            .args([
                "--server",
                "/tmp/weztcode-nvim.sock",
                "--remote",
                &full_path.to_string_lossy(),
            ])
            .output()
            .map_err(|e| format!("Failed to run nvim: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(format!("nvim --remote failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    }).await.unwrap();

    match result {
        Ok(_) => crate::api::ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}
