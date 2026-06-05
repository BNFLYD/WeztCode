use std::path::Path;

use crate::api::{json_error, json_response};

pub fn handle_editor_open(
    rel_path: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    eprintln!("[editor_open] rel_path={:?}, root={:?}", rel_path, root);

    let full_path = match crate::config::fs::sanitize_path(rel_path, root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[editor_open] sanitize_path error: {}", e);
            return json_error(&e);
        }
    };

    if !full_path.is_file() {
        return json_error("Not a file");
    }

    match std::process::Command::new("nvim")
        .args([
            "--server",
            "/tmp/weztcode-nvim.sock",
            "--remote",
            &full_path.to_string_lossy(),
        ])
        .output()
    {
        Ok(o) if o.status.success() => json_response(&serde_json::json!({ "ok": true })),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            json_error(&format!("nvim --remote failed: {}", stderr))
        }
        Err(e) => json_error(&format!("Failed to run nvim: {}", e)),
    }
}
