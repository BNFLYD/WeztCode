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
        // Lock del chat ANTES de tocar current_root: si un handler del chat
        // corriera entre el cambio de root y el respawn (p. ej. el select-scope
        // del frontend re-renderizando tras el switch), registraría el sid del
        // proceso VIEJO bajo la clave del proyecto NUEVO y el respawn
        // arrancaría una sesión huérfana con ese id contaminado.
        let mut service = crate::CHAT_SERVICE.lock().unwrap();
        crate::config::current_root::set(&path)?;
        service.restart_backend()?;
        Ok::<_, String>(())
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}
