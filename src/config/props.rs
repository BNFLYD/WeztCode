use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub struct UserProps {
    props: HashMap<String, String>,
}

impl UserProps {
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

    fn path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_default()
            .join("user_props.lua")
    }
}
