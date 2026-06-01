use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDir {
    pub name: String,
    pub path: String,
}

fn home_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\"))
    } else {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
    }
}

fn detect_path() -> PathBuf {
    home_dir().join(".config/weztcode/project_dirs.json")
}

pub fn list() -> Vec<ProjectDir> {
    let path = detect_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save(projects: &[ProjectDir]) -> Result<(), String> {
    let path = detect_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create config dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(projects)
        .map_err(|e| format!("Cannot serialize projects: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Cannot write project_dirs.json: {}", e))
}

pub fn add(path_str: &str) -> Result<Vec<ProjectDir>, String> {
    let dir = std::path::Path::new(path_str);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path_str));
    }

    let mut projects = list();

    let exists = projects.iter().any(|p| p.path == path_str);
    if exists {
        return Err("Project path already exists".to_string());
    }

    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.to_string());

    projects.push(ProjectDir {
        name,
        path: path_str.to_string(),
    });

    save(&projects)?;
    Ok(projects)
}

pub fn remove(path_str: &str) -> Result<Vec<ProjectDir>, String> {
    let mut projects = list();
    projects.retain(|p| p.path != path_str);
    save(&projects)?;
    Ok(projects)
}
