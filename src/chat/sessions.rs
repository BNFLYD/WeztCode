//! Mapa scope → session-id y helpers de sesión.
//!
//! Cada "scope" de chat (un sub-agente, o el chat "default" sin agente) tiene
//! su propia sesión de pi persistida en disco (~/.pi/agent/sessions/--<cwd>--/).
//! El mapa se guarda en ~/.config/weztcode/preferences/sessions.json para que
//! las sesiones sobrevivan restarts de la app y para que "nueva conversación"
//! no resucite la sesión archivada de ese scope.

use std::collections::HashMap;
use std::path::PathBuf;

/// Archivo del mapa scope → session-id.
fn map_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".config/weztcode/preferences/sessions.json")
}

pub struct SessionMap {
    map: HashMap<String, String>,
}

impl SessionMap {
    pub fn load() -> Self {
        let map = std::fs::read_to_string(map_path())
            .ok()
            .and_then(|c| serde_json::from_str::<HashMap<String, String>>(&c).ok())
            .unwrap_or_default();
        Self { map }
    }

    pub fn save(&self) {
        let path = map_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.map) {
            let _ = std::fs::write(path, json);
        }
    }

    /// session-id del scope; si no existe, deriva uno del nombre y lo registra.
    pub fn get_or_derive(&mut self, scope: &str) -> String {
        if let Some(sid) = self.map.get(scope) {
            return sid.clone();
        }
        let sid = sanitize_id(scope);
        self.map.insert(scope.to_string(), sid.clone());
        self.save();
        sid
    }

    pub fn set(&mut self, scope: &str, sid: String) {
        if self.map.get(scope).map(String::as_str) != Some(sid.as_str()) {
            self.map.insert(scope.to_string(), sid);
            self.save();
        }
    }
}

/// Sanea un nombre para cumplir las reglas de session-id de pi:
/// alfanumérico, '-', '_' y '.', empezando y terminando en alfanumérico.
pub fn sanitize_id(scope: &str) -> String {
    let s: String = scope
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
        .collect();
    let s = s.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if s.is_empty() { "session".to_string() } else { s.to_string() }
}

/// Directorio de sesiones de pi para el cwd actual (mismo encoding que pi:
/// `--{cwd con '/' reemplazado por '-'}--`).
pub fn project_session_dir() -> PathBuf {
    let cwd = crate::config::current_root::get();
    let resolved = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
    let s = resolved.to_string_lossy().trim_start_matches('/').replace('/', "-");
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".pi/agent/sessions").join(format!("--{}--", s))
}

/// Busca el archivo de sesión con ese id (el más reciente). None si no existe.
pub fn find_session_file(sid: &str) -> Option<PathBuf> {
    let dir = project_session_dir();
    let suffix = format!("_{}.jsonl", sid);
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&dir).ok()? {
        let path = entry.ok()?.path();
        let name = path.file_name()?.to_string_lossy().to_string();
        if name.ends_with(&suffix) {
            if let Ok(meta) = std::fs::metadata(&path) {
                if let Ok(mtime) = meta.modified() {
                    if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                        best = Some((mtime, path));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}
