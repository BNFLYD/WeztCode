use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub max_context: Option<u64>,
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

fn config_dir() -> PathBuf {
    home_dir().join(".config/weztcode/preferences/models")
}

fn detect_path() -> PathBuf {
    config_dir().join("models.json")
}

pub fn list() -> Vec<ModelEntry> {
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

pub fn get_default() -> Option<ModelEntry> {
    list().into_iter().find(|m| m.default)
}

pub fn detect_path_str() -> String {
    detect_path().to_string_lossy().to_string()
}
