use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static PROPS_DIR: OnceLock<PathBuf> = OnceLock::new();

pub struct UserProps {
    props: HashMap<String, String>,
}

impl UserProps {
    pub fn init() {
        PROPS_DIR.set(std::env::current_dir().unwrap_or_default()).ok();
    }

    pub fn load() -> Self {
        let mut props = HashMap::new();

        let path = Self::path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with("--") {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim().to_string();
                        let value = value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string();
                        props.insert(key, value);
                    }
                }
            }
        }

        Self { props }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(|s| s.as_str())
    }

    pub fn get_resolved(&self, key: &str) -> Option<String> {
        self.get(key)
            .map(|raw| crate::config::keys::KeysStore::resolve(raw))
    }

    pub fn set(key: &str, value: &str) -> Result<(), String> {
        let path = Self::path();
        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let search_key = format!("{} =", key);
        let search_key_alt = format!("{}=", key);
        let mut found = false;

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with(&search_key) || trimmed.starts_with(&search_key_alt) {
                *line = format!("{} = \"{}\"", key, value);
                found = true;
                break;
            }
        }

        if !found {
            lines.push(format!("{} = \"{}\"", key, value));
        }

        fs::write(&path, lines.join("\n") + "\n")
            .map_err(|e| format!("Cannot write user_props.lua: {}", e))
    }

    fn path() -> PathBuf {
        PROPS_DIR
            .get()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .join("user_props.lua")
    }
}
