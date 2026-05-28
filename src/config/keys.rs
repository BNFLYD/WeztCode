use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

static KEYS_PATH_OVERRIDE: &str = "WEZTCODE_KEYS_PATH";
static KEYS_PREFIX: &str = "KEYS.";

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

pub struct KeysStore {
    keys: HashMap<String, String>,
    path: PathBuf,
}

impl KeysStore {
    pub fn load() -> Self {
        let path = Self::detect_path();
        let keys = if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            Self::parse(&content)
        } else {
            HashMap::new()
        };
        Self { keys, path }
    }

    pub fn load_path() -> PathBuf {
        Self::detect_path()
    }

    fn detect_path() -> PathBuf {
        if let Ok(p) = std::env::var(KEYS_PATH_OVERRIDE) {
            return PathBuf::from(p);
        }
        let config = home_dir().join(".config/weztcode/KEYS.env");
        if config.exists() {
            return config;
        }
        let local = std::env::current_dir()
            .unwrap_or_default()
            .join("KEYS.env");
        if local.exists() {
            return local;
        }
        home_dir().join(".config/weztcode/KEYS.env")
    }

    fn parse(content: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim().to_uppercase();
                let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
                if !key.is_empty() && !val.is_empty() {
                    map.insert(key, val);
                }
            }
        }
        map
    }

    pub fn resolve(value: &str) -> String {
        let value = value.trim();
        if let Some(key_name) = value.strip_prefix(KEYS_PREFIX) {
            let store = Self::load();
            let upper = key_name.trim().to_uppercase();
            match store.keys.get(&upper) {
                Some(v) => v.clone(),
                None => {
                    eprintln!("[keys] Warning: '{}' not found in KEYS.env", key_name);
                    value.to_string()
                }
            }
        } else {
            value.to_string()
        }
    }

    pub fn set(name: &str, value: &str) -> Result<(), String> {
        let mut store = Self::load();
        let upper = name.trim().to_uppercase();
        if value.trim().is_empty() {
            store.keys.remove(&upper);
        } else {
            store.keys.insert(upper, value.trim().to_string());
        }
        store.save()
    }

    pub fn delete(name: &str) -> Result<(), String> {
        let mut store = Self::load();
        store.keys.remove(&name.trim().to_uppercase());
        store.save()
    }

    pub fn list_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.keys.keys().cloned().collect();
        names.sort();
        names
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create keys dir: {}", e))?;
        }

        let mut content = String::from("# WeztCode KEYS.env\n\n");
        let mut sorted: Vec<_> = self.keys.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        for (k, v) in &sorted {
            content.push_str(&format!("{}={}\n", k, v));
        }

        fs::write(&self.path, &content)
            .map_err(|e| format!("Cannot write KEYS.env: {}", e))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&self.path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(&self.path, perms);
            }
        }

        Ok(())
    }
}

pub fn redact_keys(input: &str) -> String {
    let store = KeysStore::load();
    let mut result = input.to_string();
    for value in store.keys.values() {
        if value.len() > 4 {
            result = result.replace(value, "***");
        }
    }
    result
}
