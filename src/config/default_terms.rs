use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultTerm {
    pub name: String,
    pub icon: String,
    pub program: String,
    #[serde(default)]
    pub autostart: bool,
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
    home_dir().join(".config/weztcode/preferences/terminals")
}

fn detect_path() -> PathBuf {
    config_dir().join("default_terms.json")
}

pub fn list() -> Vec<DefaultTerm> {
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

fn save(terms: &[DefaultTerm]) -> Result<(), String> {
    let path = detect_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create config dir: {}", e))?;
    }
    let content = serde_json::to_string_pretty(terms)
        .map_err(|e| format!("Cannot serialize default_terms: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Cannot write default_terms.json: {}", e))
}

pub fn add(name: &str, icon: &str, program: &str) -> Result<Vec<DefaultTerm>, String> {
    let mut terms = list();
    let exists = terms.iter().any(|t| t.name == name);
    if exists {
        return Err("Terminal name already exists".to_string());
    }
    terms.push(DefaultTerm {
        name: name.to_string(),
        icon: icon.to_string(),
        program: program.to_string(),
        autostart: false,
    });
    save(&terms)?;
    Ok(terms)
}

pub fn remove(name: &str) -> Result<Vec<DefaultTerm>, String> {
    let mut terms = list();
    terms.retain(|t| t.name != name);
    save(&terms)?;
    Ok(terms)
}

pub fn detect_path_str() -> String {
    detect_path().to_string_lossy().to_string()
}
