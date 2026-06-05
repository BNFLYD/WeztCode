use std::path::Path;

use crate::api::{json_error, json_response};

pub fn handle_ls(
    rel_path: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::list_dir(rel_path, root) {
        Ok(files) => {
            let display_path = if rel_path.is_empty() || rel_path == "/" {
                "/".to_string()
            } else {
                rel_path.trim_start_matches('/').to_string()
            };
            let data = serde_json::json!({
                "ok": true,
                "data": {
                    "path": display_path,
                    "files": files
                }
            });
            json_response(&data)
        }
        Err(e) => json_error(&e),
    }
}

pub fn handle_read(
    rel_path: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if rel_path.trim_start_matches('/').eq_ignore_ascii_case("KEYS.env") {
        return json_error("Access denied");
    }

    match crate::config::fs::read_file(rel_path, root) {
        Ok(content) => {
            let data = serde_json::json!({
                "ok": true,
                "data": {
                    "path": rel_path,
                    "content": content
                }
            });
            json_response(&data)
        }
        Err(e) => json_error(&e),
    }
}

pub fn handle_create(
    rel_path: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::create_entry(rel_path, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_delete(
    rel_path: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::delete_entry(rel_path, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_rename(
    rel_path: &str,
    new_name: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::rename_entry(rel_path, new_name, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_move(
    rel_path: &str,
    dest: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::move_entry(rel_path, dest, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

pub fn handle_image(
    rel_path: String,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let full_path = match crate::config::fs::sanitize_path(&rel_path, root) {
        Ok(p) => p,
        Err(e) => return json_response(&serde_json::json!({ "ok": false, "error": e })),
    };

    if !full_path.is_file() {
        return json_response(&serde_json::json!({ "ok": false, "error": "Not a file" }));
    }

    let ext = full_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let content_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };

    eprintln!(
        "[image] path={:?}, ext={:?}, content_type={}",
        rel_path, ext, content_type
    );

    match crate::config::fs::read_image_bytes(&rel_path, root) {
        Ok(bytes) => {
            let len = bytes.len();
            eprintln!("[image] read ok: {} bytes", len);
            let cursor = std::io::Cursor::new(bytes);
            tiny_http::Response::new(
                tiny_http::StatusCode(200),
                vec![tiny_http::Header {
                    field: "Content-Type".parse().unwrap(),
                    value: content_type.parse().unwrap(),
                }],
                cursor,
                Some(len),
                None,
            )
        }
        Err(e) => {
            eprintln!("[image] read error: {}", e);
            json_response(&serde_json::json!({ "ok": false, "error": e }))
        }
    }
}
