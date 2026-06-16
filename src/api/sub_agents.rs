use axum::{extract::Json, response::IntoResponse};

use crate::api::{err_json, ok_json};
use crate::config;

pub async fn handle_list() -> impl IntoResponse {
    let agents = config::sub_agents::list();
    let data: Vec<serde_json::Value> = agents
        .iter()
        .map(|a| {
            serde_json::json!({
                "name": a.name,
                "description": a.description,
                "model": a.model,
                "icon": a.icon,
                "default": a.default,
            })
        })
        .collect();
    ok_json(serde_json::json!({"ok": true, "data": data}))
}

pub async fn handle_switch(
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return err_json("Missing 'name' field"),
    };

    let result = tokio::task::spawn_blocking(move || {
        let entry = config::sub_agents::get_by_name(&name)
            .ok_or_else(|| format!("Sub-agent '{}' not found", name))?;

        let mut service = crate::CHAT_SERVICE
            .lock()
            .map_err(|e| format!("Lock: {}", e))?;

        service.switch_agent(&entry)?;

        Ok::<_, String>(serde_json::json!({
            "agent": entry.name,
            "model": entry.model,
            "description": entry.description,
            "icon": entry.icon,
        }))
    })
    .await
    .unwrap();

    match result {
        Ok(data) => ok_json(serde_json::json!({"ok": true, "data": data})),
        Err(e) => err_json(&config::keys::redact_keys(&e)),
    }
}

pub async fn handle_install() -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(|| {
        let output = std::process::Command::new("pi")
            .args(["install", "npm:pi-subagents"])
            .output()
            .map_err(|e| format!("Failed to run pi install: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("pi install failed: {}", stderr))
        }
    })
    .await
    .unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_edit() -> impl IntoResponse {
    let dirs = config::sub_agents::agent_dirs_list();
    if dirs.is_empty() {
        // Create the user agents directory if it doesn't exist
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        let user_dir = std::path::PathBuf::from(&home).join(".pi/agent/agents");
        let _ = std::fs::create_dir_all(&user_dir);
        let dir_str = user_dir.to_string_lossy().to_string();
        let result = tokio::task::spawn_blocking(move || {
            let output = std::process::Command::new("nvim")
                .args([
                    "--server",
                    "/tmp/weztcode-nvim.sock",
                    "--remote",
                    &dir_str,
                ])
                .output()
                .map_err(|e| format!("Failed to run nvim: {}", e))?;

            if output.status.success() {
                Ok(())
            } else {
                Err(format!("nvim failed: {}", String::from_utf8_lossy(&output.stderr)))
            }
        })
        .await
        .unwrap();

        return match result {
            Ok(_) => ok_json(serde_json::json!({"ok": true, "dir": user_dir.to_string_lossy()})),
            Err(e) => err_json(&e),
        };
    }

    let dir = dirs[0].clone();
    let dir_str = dir.to_string_lossy().to_string();
    let result = tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("nvim")
            .args([
                "--server",
                "/tmp/weztcode-nvim.sock",
                "--remote",
                &dir_str,
            ])
            .output()
            .map_err(|e| format!("Failed to run nvim: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(format!("nvim failed: {}", String::from_utf8_lossy(&output.stderr)))
        }
    })
    .await
    .unwrap();

    match result {
        Ok(_) => ok_json(serde_json::json!({"ok": true, "dir": dir.to_string_lossy()})),
        Err(e) => err_json(&e),
    }
}

pub async fn handle_builtins() -> impl IntoResponse {
    let builtins = config::sub_agents::builtins();
    ok_json(serde_json::json!({"ok": true, "data": builtins}))
}
