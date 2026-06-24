use std::time::Instant;

use axum::response::IntoResponse;
use axum::Json;
use regex::Regex;
use serde_json::{json, Value};

use crate::api::{err_json, get_current_root};

struct ToolDef {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    command: &'static str,
    args: &'static [&'static str],
}

static TOOLS: &[ToolDef] = &[
    ToolDef {
        id: "cargo_test",
        name: "Cargo Test",
        description: "Ejecuta cargo test y parsea resultados",
        icon: "tabler:test-pipe",
        command: "cargo",
        args: &["test", "--color", "never", "--format", "json"],
    },
    ToolDef {
        id: "cargo_clippy",
        name: "Clippy",
        description: "Ejecuta cargo clippy y muestra advertencias/errores",
        icon: "tabler:alert-triangle",
        command: "cargo",
        args: &["clippy", "--color", "never", "--message-format", "human"],
    },
    ToolDef {
        id: "cargo_check",
        name: "Cargo Check",
        description: "Ejecuta cargo check y muestra errores de compilación",
        icon: "tabler:build",
        command: "cargo",
        args: &["check", "--color", "never", "--message-format", "human"],
    },
];

pub async fn handle_tools() -> impl IntoResponse {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| {
            json!({
                "id": t.id,
                "name": t.name,
                "description": t.description,
                "icon": t.icon
            })
        })
        .collect();

    crate::api::ok_json(json!({"ok": true, "data": { "tools": tools }}))
}

pub async fn handle_run(Json(body): Json<Value>) -> impl IntoResponse {
    let tool_id = body
        .get("tool_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tool = TOOLS.iter().find(|t| t.id == tool_id);
    let tool = match tool {
        Some(t) => t,
        None => return err_json(&format!("Unknown tool: {}", tool_id)),
    };

    let root = get_current_root();
    let command = tool.command;
    let args: Vec<&str> = tool.args.to_vec();

    let result = tokio::task::spawn_blocking(move || {
        let start = Instant::now();

        let output = std::process::Command::new(command)
            .args(&args)
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Failed to run {}: {}", tool_id, e))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let items = match tool_id.as_str() {
            "cargo_test" => parse_cargo_test(&stdout, &stderr),
            "cargo_clippy" | "cargo_check" => parse_cargo_diagnostics(&stderr),
            _ => vec![],
        };

        let summary = compute_summary(&items);

        Ok::<_, String>(json!({
            "tool_id": tool_id,
            "success": output.status.success(),
            "duration_ms": duration_ms,
            "items": items,
            "summary": summary
        }))
    })
    .await
    .unwrap();

    match result {
        Ok(data) => crate::api::ok_json(json!({"ok": true, "data": data})),
        Err(e) => err_json(&e),
    }
}

fn parse_cargo_test(stdout: &str, _stderr: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Ok(ev) = serde_json::from_str::<Value>(line) {
            let ev_type = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let event = ev.get("event").and_then(|v| v.as_str()).unwrap_or("");

            if ev_type == "test" {
                let suite = ev
                    .get("suite")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = ev
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let (status, message) = match event {
                    "ok" => ("pass", None),
                    "failed" => {
                        let msg = ev
                            .get("stdout")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        ("fail", msg)
                    }
                    "ignored" => ("ignored", None),
                    _ => continue,
                };

                items.push(json!({
                    "type": "test",
                    "status": status,
                    "suite": suite,
                    "name": name,
                    "file": null,
                    "line": null,
                    "col": null,
                    "message": message
                }));
            } else if ev_type == "suite" && event == "failed" {
                if let Some(ev_stdout) = ev.get("stdout").and_then(|v| v.as_str()) {
                    if let Some(captures) = extract_test_failure_location(ev_stdout, &items) {
                        if let Some(last) = items.last_mut() {
                            if last.get("file").and_then(|v| v.as_str()).unwrap_or("").is_empty()
                            {
                                *last = captures;
                            }
                        }
                    }
                }
            }
        }
    }

    items
}

fn extract_test_failure_location(output: &str, items: &[Value]) -> Option<Value> {
    let re = Regex::new(r"(?m)^\s+-->?\s+(.+):(\d+):(\d+)").ok()?;

    for cap in re.captures_iter(output) {
        let file = cap.get(1)?.as_str().to_string();
        let line: i64 = cap.get(2)?.as_str().parse().ok()?;
        let col: i64 = cap.get(3)?.as_str().parse().ok()?;

        let last_item = items.last()?;
        return Some(json!({
            "type": last_item.get("type"),
            "status": last_item.get("status"),
            "suite": last_item.get("suite"),
            "name": last_item.get("name"),
            "file": file,
            "line": line,
            "col": col,
            "message": last_item.get("message")
        }));
    }

    None
}

fn parse_cargo_diagnostics(stderr: &str) -> Vec<Value> {
    let mut items: Vec<Value> = Vec::new();
    let loc_re = Regex::new(r"^\s+-->\s+(.+):(\d+):(\d+)").unwrap();
    let diag_start = Regex::new(r"^(error|warning)(?:\[([A-Za-z0-9_]+)\])?\s*:\s*(.*)")
        .unwrap();

    let mut current_severity: Option<String> = None;
    let mut current_code: Option<String> = None;
    let mut current_message: String = String::new();
    let mut current_file: Option<String> = None;
    let mut current_line: Option<i64> = None;
    let mut current_col: Option<i64> = None;
    let mut in_diagnostic = false;

    for line in stderr.lines() {
        if line.is_empty() {
            flush_diagnostic(
                &mut items,
                &mut in_diagnostic,
                &mut current_severity,
                &mut current_code,
                &mut current_message,
                &mut current_file,
                &mut current_line,
                &mut current_col,
            );
            continue;
        }

        if let Some(caps) = diag_start.captures(line) {
            flush_diagnostic(
                &mut items,
                &mut in_diagnostic,
                &mut current_severity,
                &mut current_code,
                &mut current_message,
                &mut current_file,
                &mut current_line,
                &mut current_col,
            );

            current_severity = Some(caps.get(1).unwrap().as_str().to_string());
            current_code = caps.get(2).map(|m| m.as_str().to_string());
            current_message = caps.get(3).unwrap().as_str().to_string();
            in_diagnostic = true;
            continue;
        }

        if let Some(caps) = loc_re.captures(line) {
            current_file = Some(caps.get(1).unwrap().as_str().to_string());
            current_line = caps.get(2).unwrap().as_str().parse().ok();
            current_col = caps.get(3).unwrap().as_str().parse().ok();
            continue;
        }

        if in_diagnostic {
            let trimmed = line.trim();
            if trimmed.starts_with('=') {
                let note = trimmed.trim_start_matches('=').trim();
                if !current_message.is_empty() {
                    current_message.push('\n');
                }
                current_message.push_str(note);
            } else if !trimmed.starts_with('|') {
                if !current_message.is_empty() {
                    current_message.push('\n');
                }
                current_message.push_str(trimmed);
            }
        }
    }

    flush_diagnostic(
        &mut items,
        &mut in_diagnostic,
        &mut current_severity,
        &mut current_code,
        &mut current_message,
        &mut current_file,
        &mut current_line,
        &mut current_col,
    );

    items
}

#[allow(clippy::too_many_arguments)]
fn flush_diagnostic(
    items: &mut Vec<Value>,
    in_diagnostic: &mut bool,
    severity: &mut Option<String>,
    code: &mut Option<String>,
    message: &mut String,
    file: &mut Option<String>,
    line: &mut Option<i64>,
    col: &mut Option<i64>,
) {
    if !*in_diagnostic {
        return;
    }

    let sev = severity.take().unwrap_or_default();
    let msg = std::mem::take(message);
    let f = file.take();
    let l = line.take();
    let c = col.take();
    let cd = code.take();

    items.push(json!({
        "type": "diagnostic",
        "severity": sev,
        "file": f,
        "line": l,
        "col": c,
        "message": msg,
        "code": cd
    }));

    *in_diagnostic = false;
}

fn compute_summary(items: &[Value]) -> Value {
    let total = items.len();
    let mut passed = 0;
    let mut failed = 0;
    let mut errors = 0;
    let mut warnings = 0;

    for item in items {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match item_type {
            "test" => match item.get("status").and_then(|v| v.as_str()) {
                Some("pass") => passed += 1,
                Some("fail") => failed += 1,
                _ => {}
            },
            "diagnostic" => match item.get("severity").and_then(|v| v.as_str()) {
                Some("error") => errors += 1,
                Some("warning") => warnings += 1,
                _ => {}
            },
            _ => {}
        }
    }

    json!({
        "total": total,
        "passed": passed,
        "failed": failed,
        "errors": errors,
        "warnings": warnings
    })
}
