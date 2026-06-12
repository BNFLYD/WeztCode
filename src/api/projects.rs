use std::collections::HashMap;

use axum::extract::{Json, Query};
use axum::response::IntoResponse;

use crate::api::{err_json, ok_json};

pub async fn handle_projects_list() -> impl IntoResponse {
    let projects = crate::config::project_dirs::list();
    ok_json(serde_json::json!({"ok": true, "data": projects}))
}

pub async fn handle_projects_add(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let path = body.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let result = tokio::task::spawn_blocking(move || {
        crate::config::project_dirs::add(&path)
    }).await.unwrap();

    match result {
        Ok(projects) => ok_json(serde_json::json!({"ok": true, "data": projects})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_projects_delete(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let path = params.get("path").map(|s| s.to_string()).unwrap_or_default();

    let result = tokio::task::spawn_blocking(move || {
        crate::config::project_dirs::remove(&path)
    }).await.unwrap();

    match result {
        Ok(projects) => ok_json(serde_json::json!({"ok": true, "data": projects})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_projects_switch(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let path = params.get("path").map(|s| s.to_string()).unwrap_or_default();

    let result = tokio::task::spawn_blocking(move || {
        let dir = std::path::Path::new(&path);
        if !dir.is_dir() {
            return Err("Directory not found".to_string());
        }
        std::env::set_current_dir(dir)
            .map_err(|e| format!("Failed to set cwd: {}", e))?;
        crate::config::props::UserProps::set("current_dir", &path)
            .map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}
