use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermMetadata {
    pub name: Option<String>,
    pub icon: Option<String>,
}

pub type MetadataMap = HashMap<u32, TermMetadata>;

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
    home_dir().join(".config/weztcode/preferences/terminals/terms_metadata.json")
}

fn load_raw() -> MetadataMap {
    let path = detect_path();
    if !path.exists() {
        return HashMap::new();
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_raw(map: &MetadataMap) -> Result<(), String> {
    let path = detect_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create config dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(map)
        .map_err(|e| format!("Cannot serialize terms_metadata: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Cannot write terms_metadata.json: {}", e))
}

pub fn list() -> MetadataMap {
    load_raw()
}

pub fn set(pane_id: u32, name: Option<String>, icon: Option<String>) -> Result<(), String> {
    let mut map = load_raw();
    map.insert(pane_id, TermMetadata { name, icon });
    save_raw(&map)
}

pub fn remove(pane_id: u32) -> Result<(), String> {
    let mut map = load_raw();
    map.remove(&pane_id);
    save_raw(&map)
}
