use std::collections::HashMap;

use axum::extract::{Json, Query};
use axum::response::IntoResponse;

use crate::api::{err_json, ok_json};
use crate::terminal::{TerminalProtocol, WeztermProtocol};

pub async fn handle_terminal_list() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let term = WeztermProtocol::new();
        let panes = term.list_panes_structured()?;
        let metadata = crate::config::terms_metadata::list();
        Ok::<_, String>((panes, metadata))
    }).await.unwrap();

    match result {
        Ok((panes, metadata)) => ok_json(serde_json::json!({
            "ok": true,
            "data": { "panes": panes, "metadata": metadata }
        })),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_spawn(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let name = body.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let icon = body.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string());
    let program = body.get("program").and_then(|v| v.as_str()).map(|s| s.to_string());

    let result = tokio::task::spawn_blocking(move || {
        let cwd = crate::config::props::UserProps::load()
            .get("current_dir")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let term = WeztermProtocol::new();
        let pane_id = term.spawn_tab(cwd.as_deref(), program.as_deref())?;

        if name.is_some() || icon.is_some() {
            let _ = crate::config::terms_metadata::set(pane_id, name.clone(), icon.clone());
        }
        Ok::<_, String>(serde_json::json!({
            "pane_id": pane_id,
            "name": name,
            "icon": icon
        }))
    }).await.unwrap();

    match result {
        Ok(data) => ok_json(serde_json::json!({"ok": true, "data": data})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_kill(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let pane_id = params.get("pane_id")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    if pane_id == 0 { return err_json("Invalid pane_id"); }

    let result = tokio::task::spawn_blocking(move || {
        let term = WeztermProtocol::new();
        term.kill_pane(pane_id)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_activate(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let pane_id = params.get("pane_id")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    if pane_id == 0 { return err_json("Invalid pane_id"); }

    let result = tokio::task::spawn_blocking(move || {
        let term = WeztermProtocol::new();
        term.activate_pane(pane_id)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_metadata_get() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        crate::config::terms_metadata::list()
    }).await.unwrap();
    ok_json(serde_json::json!({"ok": true, "data": result}))
}

pub async fn handle_terminal_metadata_set(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let pane_id = body.get("pane_id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let name = body.get("name").and_then(|v| v.as_str()).map(String::from);
    let icon = body.get("icon").and_then(|v| v.as_str()).map(String::from);

    let result = tokio::task::spawn_blocking(move || {
        crate::config::terms_metadata::set(pane_id, name, icon)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_metadata_delete(
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let pane_id = params.get("pane_id")
        .and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);

    let result = tokio::task::spawn_blocking(move || {
        crate::config::terms_metadata::remove(pane_id)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_edit_defaults() -> impl IntoResponse {
    let path = crate::config::default_terms::detect_path_str();
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

pub async fn handle_terminal_default_terms() -> impl IntoResponse {
    let terms = crate::config::default_terms::list();
    ok_json(serde_json::json!({"ok": true, "data": terms}))
}

pub async fn handle_active_pane() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let path = std::env::var("WEZTCODE_ACTIVE_PANE_FILE")
            .map_err(|_| "ACTIVE_PANE_FILE not set".to_string())?;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read active_pane: {}", e))?;
        let pane_id = content.trim().parse::<u32>().unwrap_or(0);
        Ok::<_, String>(pane_id)
    }).await.unwrap();

    match result {
        Ok(pane_id) => ok_json(serde_json::json!({"ok": true, "data": {"pane_id": pane_id}})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_terminal_ensure_main() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let term = WeztermProtocol::new();
        term.activate_pane(0)
    }).await.unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}
