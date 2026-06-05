use std::collections::HashMap;

use axum::extract::{Json, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::api::{err_json, get_current_root};

pub async fn handle_ls(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let rel_path = params.get("path").map(|s| s.to_string()).unwrap_or_else(|| "/".to_string());

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        let files = crate::config::fs::list_dir(&rel_path, &root)?;
        let display_path = if rel_path.is_empty() || rel_path == "/" {
            "/".to_string()
        } else {
            rel_path.trim_start_matches('/').to_string()
        };
        Ok::<_, String>(serde_json::json!({
            "path": display_path,
            "files": files
        }))
    }).await.unwrap();

    match result {
        Ok(data) => crate::api::ok_json(serde_json::json!({"ok": true, "data": data})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_read(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let rel_path = params.get("path").map(|s| s.to_string()).unwrap_or_default();

    if rel_path.trim_start_matches('/').eq_ignore_ascii_case("KEYS.env") {
        return err_json("Access denied");
    }

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        let content = crate::config::fs::read_file(&rel_path, &root)?;
        Ok::<_, String>(serde_json::json!({
            "path": rel_path,
            "content": content
        }))
    }).await.unwrap();

    match result {
        Ok(data) => crate::api::ok_json(serde_json::json!({"ok": true, "data": data})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_create(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rel_path = body.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        crate::config::fs::create_entry(&rel_path, &root)
    }).await.unwrap();

    match result {
        Ok(_) => crate::api::ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_delete(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let rel_path = params.get("path").map(|s| s.to_string()).unwrap_or_default();

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        crate::config::fs::delete_entry(&rel_path, &root)
    }).await.unwrap();

    match result {
        Ok(_) => crate::api::ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_rename(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rel_path = body.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let new_name = body.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        crate::config::fs::rename_entry(&rel_path, &new_name, &root)
    }).await.unwrap();

    match result {
        Ok(_) => crate::api::ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_move(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let rel_path = body.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let dest = body.get("dest").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        crate::config::fs::move_entry(&rel_path, &dest, &root)
    }).await.unwrap();

    match result {
        Ok(_) => crate::api::ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_image(
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let rel_path = params.get("path").map(|s| s.to_string()).unwrap_or_default();

    let result = tokio::task::spawn_blocking(move || {
        let root = get_current_root();
        let full_path = crate::config::fs::sanitize_path(&rel_path, &root)?;
        if !full_path.is_file() { return Err("Not a file".to_string()); }

        let ext = full_path.extension()
            .and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
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

        let bytes = crate::config::fs::read_image_bytes(&rel_path, &root)?;
        Ok::<_, String>((content_type.to_string(), bytes))
    }).await.unwrap();

    match result {
        Ok((content_type, bytes)) => {
            (StatusCode::OK, [(header::CONTENT_TYPE, content_type.as_str())], bytes).into_response()
        }
        Err(e) => err_json(&e).into_response(),
    }
}
