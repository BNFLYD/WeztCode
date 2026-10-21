//! Mapa scope → session-id y helpers de sesión.
//!
//! Cada "scope" de chat (un sub-agente, o el chat "default" sin agente) tiene
//! su propia sesión de pi persistida en disco (~/.pi/agent/sessions/--<cwd>--/).
//! El mapa se guarda en ~/.config/weztcode/preferences/sessions.json para que
//! las sesiones sobrevivan restarts de la app y para que "nueva conversación"
//! no resucite la sesión archivada de ese scope.
//!
//! La clave del mapa es compuesta: "{proyecto}::{scope}". Las sesiones de pi
//! viven en un directorio POR PROYECTO (--{cwd}--), así que un mapa global
//! plano hacía que cambiar de proyecto creara conversaciones nuevas vacías:
//! el id mapeado solo existía en el directorio del proyecto anterior y el
//! respawn con --session-id generaba una sesión vacía en el proyecto actual
//! (mientras la conversación real quedaba huérfana en el disco). Con claves
//! por-proyecto, cada proyecto recuerda SU sesión de cada agente.

use std::collections::HashMap;
use std::path::PathBuf;

/// Archivo del mapa scope → session-id.
fn map_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_default();
    PathBuf::from(home).join(".config/weztcode/preferences/sessions.json")
}

/// Clave compuesta "{proyecto}::{scope}" para el mapa por-proyecto.
fn composite_key(scope: &str) -> String {
    let project = crate::config::current_root::get();
    format!("{}::{}", project.to_string_lossy(), scope)
}

pub struct SessionMap {
    map: HashMap<String, String>,
}

impl SessionMap {
    pub fn load() -> Self {
        let mut map = std::fs::read_to_string(map_path())
            .ok()
            .and_then(|c| serde_json::from_str::<HashMap<String, String>>(&c).ok())
            .unwrap_or_default();
        // Migración del formato viejo (claves planas, mapa global): el
        // proyecto actual reclama las entradas huérfanas una única vez y se
        // persiste el resultado. Es la mejor aproximación: el último id
        // guardado pertenecía a lo que la app tenía como sesión viva.
        let legacy: Vec<(String, String)> = map
            .iter()
            .filter(|(k, _)| !k.contains("::"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !legacy.is_empty() {
            for (k, v) in legacy {
                map.remove(&k);
                let scope = k;
                map.insert(composite_key(&scope), v);
            }
            let new = Self { map };
            new.save();
            return new;
        }
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

    /// session-id del scope en el proyecto actual; si no existe, deriva uno
    /// del nombre y lo registra.
    pub fn get_or_derive(&mut self, scope: &str) -> String {
        let key = composite_key(scope);
        if let Some(sid) = self.map.get(&key) {
            return sid.clone();
        }
        let sid = sanitize_id(scope);
        self.map.insert(key, sid.clone());
        self.save();
        sid
    }

    pub fn set(&mut self, scope: &str, sid: String) {
        let key = composite_key(scope);
        if self.map.get(&key).map(String::as_str) != Some(sid.as_str()) {
            self.map.insert(key, sid);
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

/// Metadatos de una conversación del proyecto actual (para el picker de la UI).
pub struct ConversationInfo {
    pub id: String,
    pub file: String,
    /// Timestamp de creación (del header de sesión).
    pub created: String,
    /// Última modificación del archivo (epoch ms, la UI ordena/formatea).
    pub modified_ms: u64,
    pub size: u64,
    /// Mensajes user+assistant (conteo aproximado por substring).
    pub messages: u64,
    /// Preview: primer mensaje REAL del usuario (salta el prompt inyectado).
    pub preview: Option<String>,
    /// Agente dueño de la conversación (detectado del prompt inyectado).
    pub agent: Option<String>,
}

/// Lista todas las conversaciones del proyecto actual, la más reciente
/// primero. Lee los .jsonl del directorio de sesiones de pi sin tocar el
/// proceso vivo.
pub fn list_conversations() -> Vec<ConversationInfo> {
    use std::io::BufRead;

    let dir = project_session_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut out: Vec<ConversationInfo> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else { continue };
        let meta = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut id = String::new();
        let mut created = String::new();
        let mut messages: u64 = 0;
        // Primer y segundo mensaje de usuario: si el primero es el prompt
        // inyectado del agente, el preview sale del segundo (el mensaje real).
        let mut first_user: Option<String> = None;
        let mut second_user: Option<String> = None;

        let mut reader = std::io::BufReader::new(file);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let t = line.trim();
            if t.starts_with("{\"type\":\"session\"") {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(t) {
                    id = v
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                    created = v
                        .get("timestamp")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string();
                }
            } else if t.contains("\"type\":\"message\"") {
                let is_user = t.contains("\"role\":\"user\"");
                if is_user || t.contains("\"role\":\"assistant\"") {
                    messages += 1;
                }
                if is_user && second_user.is_none() {
                    if first_user.is_none() {
                        first_user = extract_user_text(t);
                    } else {
                        second_user = extract_user_text(t);
                    }
                }
            }
        }

        // Fallback del id: el filename es {timestamp}_{id}.jsonl.
        if id.is_empty() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(stripped) = name.strip_suffix(".jsonl") {
                    if let Some(pos) = stripped.rfind('_') {
                        id = stripped[pos + 1..].to_string();
                    }
                }
            }
        }
        if id.is_empty() {
            continue; // archivo sin id usable: ignorar
        }

        // Preview + agente: el prompt inyectado empieza con "[System instructions]".
        let (preview, agent) = match (&first_user, &second_user) {
            (Some(f), s) if f.starts_with("[System instructions]") => {
                let agent = f
                    .trim_start_matches("[System instructions]")
                    .trim_start()
                    .strip_prefix("Tu nombre es ")
                    .map(|r| r.split(',').next().unwrap_or(r).trim().to_string());
                (s.clone().map(|x| truncate(&x, 100)), agent.filter(|a| !a.is_empty()))
            }
            (Some(f), _) => (Some(truncate(f, 100)), None),
            (None, _) => (None, None),
        };

        out.push(ConversationInfo {
            id,
            file: path.to_string_lossy().to_string(),
            created,
            modified_ms,
            size: meta.len(),
            messages,
            preview,
            agent,
        });
    }

    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out
}

/// Extrae el texto del content de un registro de mensaje de usuario.
fn extract_user_text(line: &str) -> Option<String> {
    let v = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let content = v.get("message")?.get("content")?;
    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => Some(
            arr.iter()
                .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(""),
        ),
        _ => None,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{}…", t)
    }
}
