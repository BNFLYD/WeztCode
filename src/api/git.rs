use axum::response::IntoResponse;
use serde_json::{json, Value};

use crate::api::{err_json, get_current_root};

pub async fn handle_git_log() -> impl IntoResponse {
    let root = get_current_root();

    let result = tokio::task::spawn_blocking(move || {
        let output = std::process::Command::new("git")
            .args([
                "log",
                "--all",
                "--format=COMMIT%n%H%n%h%n%an%n%ae%n%ad%n%s%n%P",
                "--numstat",
                "--date=short",
                "-50",
            ])
            .current_dir(&root)
            .output()
            .map_err(|e| format!("Failed to run git log: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stderr.contains("not a git repository") {
                return Err("Not a git repository".to_string());
            }
            return Err(stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let commits = parse_git_log(&stdout);

        Ok::<_, String>(json!({ "commits": commits }))
    })
    .await
    .unwrap();

    match result {
        Ok(data) => crate::api::ok_json(json!({"ok": true, "data": data})),
        Err(e) => err_json(&e),
    }
}

fn parse_git_log(output: &str) -> Vec<Value> {
    let mut commits: Vec<Value> = Vec::new();
    let mut lines = output.lines().peekable();

    while let Some(line) = lines.next() {
        if line.trim() != "COMMIT" {
            continue;
        }

        let hash = lines.next().unwrap_or("").to_string();
        let short_hash = lines.next().unwrap_or("").to_string();
        let author = lines.next().unwrap_or("").to_string();
        let _email = lines.next().unwrap_or("").to_string();
        let date = lines.next().unwrap_or("").to_string();
        let message = lines.next().unwrap_or("").to_string();
        let parents_str = lines.next().unwrap_or("").to_string();

        let parents: Vec<&str> = parents_str.split_whitespace().filter(|s| !s.is_empty()).collect();
        let is_merge = parents.len() > 1;

        let mut files: Vec<Value> = Vec::new();

        while let Some(fline) = lines.next() {
            if fline.trim().is_empty() {
                break;
            }
            if fline.starts_with("COMMIT") {
                break;
            }

            let parts: Vec<&str> = fline.splitn(3, '\t').collect();
            if parts.len() == 3 {
                if parts[0] == "-" && parts[1] == "-" {
                    files.push(json!({
                        "path": parts[2],
                        "additions": null,
                        "deletions": null,
                    }));
                } else {
                    let additions: i64 = parts[0].parse().unwrap_or(0);
                    let deletions: i64 = parts[1].parse().unwrap_or(0);
                    files.push(json!({
                        "path": parts[2],
                        "additions": additions,
                        "deletions": deletions,
                    }));
                }
            }
        }

        commits.push(json!({
            "hash": hash,
            "short_hash": short_hash,
            "author": author,
            "date": date,
            "message": message,
            "is_merge": is_merge,
            "files": files,
        }));
    }

    commits
}
