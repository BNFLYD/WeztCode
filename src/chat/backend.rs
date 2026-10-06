use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Sabor del agente: pi vanilla o little-coder (wrapper optimizado para modelos chicos).
/// Ambos hablan el mismo protocolo RPC JSONL; cambia el binario y los flags de lanzamiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendFlavor {
    Pi,
    LittleCoder,
}

impl BackendFlavor {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "little-coder" | "littlecoder" | "lc" => Self::LittleCoder,
            _ => Self::Pi,
        }
    }

    /// Parse estricto para el campo `engine` de agentes: a diferencia de
    /// `from_str`, un valor desconocido devuelve None en lugar de caer en Pi.
    pub fn parse_strict(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pi" => Some(Self::Pi),
            "little-coder" | "littlecoder" | "lc" => Some(Self::LittleCoder),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::LittleCoder => "little-coder",
        }
    }
}

#[derive(Clone)]
pub struct ChatConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub pi_path: String,
    pub thinking_level: Option<String>,
    pub flavor: BackendFlavor,
    pub lc_path: String,
}

impl ChatConfig {
    pub fn from_default_model() -> Self {
        if let Some(model) = crate::config::models::get_default() {
            let resolved_key = crate::config::keys::KeysStore::resolve(&model.api_key);
            Self {
                provider: model.provider,
                model: model.model,
                api_key: resolved_key,
                pi_path: find_pi_path(),
                thinking_level: model.thinking_level.clone(),
                flavor: default_flavor(),
                lc_path: find_little_coder_path(),
            }
        } else {
            Self::from_props()
        }
    }

    pub fn from_model_entry(entry: &crate::config::models::ModelEntry) -> Self {
        Self {
            provider: entry.provider.clone(),
            model: entry.model.clone(),
            api_key: crate::config::keys::KeysStore::resolve(&entry.api_key),
            pi_path: find_pi_path(),
            thinking_level: entry.thinking_level.clone(),
            flavor: default_flavor(),
            lc_path: find_little_coder_path(),
        }
    }

    pub fn from_sub_agent(entry: &crate::config::sub_agents::SubAgentEntry) -> Self {
        // Intentar resolver por name en models.json (case-insensitive, trimmed)
        let models = crate::config::models::list();
        let agent_model = entry.model.trim();
        if let Some(model_entry) = models.iter().find(|m| m.name.trim().eq_ignore_ascii_case(agent_model)) {
            let resolved_key = crate::config::keys::KeysStore::resolve(&model_entry.api_key);
            return Self {
                provider: model_entry.provider.clone(),
                model: model_entry.model.clone(),
                api_key: resolved_key,
                pi_path: find_pi_path(),
                thinking_level: model_entry.thinking_level.clone(),
                flavor: default_flavor(),
                lc_path: find_little_coder_path(),
            };
        }

        // Fallback: usar provider y api_key de user_props.lua
        let props = crate::config::props::UserProps::load();
        let provider = props.get("llm_provider").unwrap_or("openrouter").to_string();
        let api_key = props.get_resolved("llm_api_key").unwrap_or_default();
        Self {
            provider,
            model: entry.model.clone(),
            api_key,
            pi_path: find_pi_path(),
            thinking_level: None,
            flavor: default_flavor(),
            lc_path: find_little_coder_path(),
        }
    }

    pub fn from_props() -> Self {
        let props = crate::config::props::UserProps::load();
        Self {
            provider: props.get("llm_provider").unwrap_or("openrouter").to_string(),
            model: props.get("llm_model").unwrap_or("openrouter/anthropic/claude-sonnet-4").to_string(),
            api_key: props.get_resolved("llm_api_key").unwrap_or_default(),
            pi_path: find_pi_path(),
            thinking_level: None,
            flavor: default_flavor(),
            lc_path: find_little_coder_path(),
        }
    }
}

pub fn find_pi_path() -> String {
    let props = crate::config::props::UserProps::load();
    if let Some(path) = props.get("pi_path").filter(|s| !s.is_empty()) {
        let resolved = path.to_string();
        // eprintln!("[pi] find_pi_path: explicit pi_path = {}", resolved);
        return resolved;
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{}/.local/share/pnpm/pi", home),
        format!("{}/.npm-global/bin/pi", home),
        "/usr/local/bin/pi".to_string(),
        "/usr/bin/pi".to_string(),
    ];

    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            // eprintln!("[pi] find_pi_path: found at {}", candidate);
            return candidate.to_string();
        }
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let local_pi = cwd.join("node_modules/.bin/pi");
    if local_pi.exists() {
        let resolved = local_pi.to_string_lossy().to_string();
        // eprintln!("[pi] find_pi_path: found local at {}", resolved);
        return resolved;
    }

    // eprintln!("[pi] find_pi_path: fallback to 'pi' (PATH lookup)");
    "pi".to_string()
}

/// Detecta el binario little-coder. Devuelve None si no está instalado.
/// Orden: prop lc_path → candidatos fijos (mismo layout que pi) → escaneo de PATH.
pub fn find_little_coder_binary() -> Option<String> {
    let props = crate::config::props::UserProps::load();
    if let Some(path) = props.get("lc_path").map(str::trim).filter(|s| !s.is_empty()) {
        return Some(path.to_string());
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{}/.local/share/pnpm/little-coder", home),
        format!("{}/.npm-global/bin/little-coder", home),
        "/usr/local/bin/little-coder".to_string(),
        "/usr/bin/little-coder".to_string(),
    ];

    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return Some(candidate.clone());
        }
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let local_lc = cwd.join("node_modules/.bin/little-coder");
    if local_lc.exists() {
        return Some(local_lc.to_string_lossy().to_string());
    }

    if let Ok(paths) = std::env::var("PATH") {
        for dir in paths.split(':') {
            let p = PathBuf::from(dir).join("little-coder");
            if p.is_file() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }

    None
}

pub fn find_little_coder_path() -> String {
    find_little_coder_binary().unwrap_or_else(|| "little-coder".to_string())
}

/// Sabor por defecto del backend:
/// 1. Prop explícita agent_backend ("pi" | "little-coder") en user_props.lua
/// 2. Auto: little-coder si el binario está instalado, si no pi
pub fn default_flavor() -> BackendFlavor {
    let props = crate::config::props::UserProps::load();
    match props.get("agent_backend").map(str::trim).filter(|s| !s.is_empty()) {
        Some(v) => BackendFlavor::from_str(v),
        None => {
            if find_little_coder_binary().is_some() {
                BackendFlavor::LittleCoder
            } else {
                BackendFlavor::Pi
            }
        }
    }
}

pub fn sync_pi_model_overrides() -> Result<(), String> {
    let models = crate::config::models::list();
    let with_reasoning: Vec<_> = models.iter()
        .filter(|m| m.reasoning.unwrap_or(false))
        .collect();

    if with_reasoning.is_empty() {
        return Ok(());
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "HOME/USERPROFILE not set".to_string())?;
    let pi_dir = PathBuf::from(&home).join(".pi/agent");
    let pi_models_path = pi_dir.join("models.json");

    let mut config: serde_json::Value = if pi_models_path.exists() {
        let content = fs::read_to_string(&pi_models_path)
            .map_err(|e| format!("Failed to read {}: {}", pi_models_path.display(), e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !config.is_object() {
        config = serde_json::json!({});
    }
    if config.get("providers").is_none() {
        config["providers"] = serde_json::json!({});
    }

    for model in &with_reasoning {
        let provider = &model.provider;
        let model_name = &model.model;

        let provider_entry = config["providers"]
            .as_object_mut()
            .ok_or_else(|| "providers not an object".to_string())?
            .entry(provider.clone())
            .or_insert_with(|| serde_json::json!({}));
        let provider_obj = provider_entry.as_object_mut()
            .ok_or_else(|| format!("provider {} entry not an object", provider))?;

        let overrides = provider_obj.entry("modelOverrides")
            .or_insert_with(|| serde_json::json!({}));
        let overrides_obj = overrides.as_object_mut()
            .ok_or_else(|| "modelOverrides not an object".to_string())?;

        let model_entry = overrides_obj.entry(model_name.clone())
            .or_insert_with(|| serde_json::json!({}));
        let model_obj = model_entry.as_object_mut()
            .ok_or_else(|| format!("model {} entry not an object", model_name))?;

        model_obj.insert("reasoning".to_string(), serde_json::json!(true));
    }

    fs::create_dir_all(&pi_dir)
        .map_err(|e| format!("Failed to create {}: {}", pi_dir.display(), e))?;

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("JSON serialize: {}", e))?;
    fs::write(&pi_models_path, &content)
        .map_err(|e| format!("Failed to write {}: {}", pi_models_path.display(), e))?;

    // eprintln!("[pi] synced model overrides to {}", pi_models_path.display());
    Ok(())
}

static RPC_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Envía un comando RPC con `id` único y lee líneas hasta encontrar la respuesta
/// con ese id. pi procesa comandos de forma asíncrona y puede emitir respuestas
/// fuera de orden, además de eventos (`extension_ui_request`, notificaciones)
/// en cualquier momento: sin correlación por id, el primer línea disponible se
/// interpretaba como la respuesta y el protocolo se desincronizaba.
fn rpc_exchange(
    stdin: &Arc<Mutex<ChildStdin>>,
    stdout: &Arc<Mutex<ChildStdout>>,
    command: &str,
    mut payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let id = format!(
        "weztcode-{}",
        RPC_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    payload["id"] = serde_json::Value::String(id.clone());
    let msg_str =
        serde_json::to_string(&payload).map_err(|e| format!("JSON serialize: {}", e))?;

    {
        let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
        writeln!(stdin_lock, "{}", msg_str)
            .map_err(|e| format!("Failed to write {}: {}", command, e))?;
        stdin_lock
            .flush()
            .map_err(|e| format!("Failed to flush {}: {}", command, e))?;
    }

    let mut stdout_lock = stdout.lock().map_err(|e| format!("stdout lock: {}", e))?;
    let mut discarded = 0u32;
    // Techo de seguridad: si nunca llega la respuesta, cortar en vez de colgar.
    const MAX_DISCARDED: u32 = 4096;

    loop {
        let mut bytes = Vec::new();
        let mut buf = [0u8; 1];
        loop {
            match stdout_lock.read(&mut buf) {
                Ok(0) => {
                    return Err(format!(
                        "RPC {}: stdout EOF antes de la respuesta ({} líneas descartadas)",
                        command, discarded
                    ))
                }
                Ok(_) => {
                    if buf[0] == b'\n' {
                        break;
                    }
                    bytes.push(buf[0]);
                }
                Err(e) => return Err(format!("stdout read error in {}: {}", command, e)),
            }
        }
        if bytes.is_empty() {
            continue;
        }
        let line = String::from_utf8_lossy(&bytes);
        let parsed = serde_json::from_str::<serde_json::Value>(line.trim());
        let Ok(json) = parsed else {
            discarded += 1;
            eprintln!(
                "[rpc] {} descartando línea no-JSON (#{}): {}",
                command,
                discarded,
                &line[..line.len().min(200)]
            );
            continue;
        };
        let is_ours = json.get("type").and_then(|v| v.as_str()) == Some("response")
            && json.get("id").and_then(|v| v.as_str()) == Some(id.as_str());
        if is_ours {
            return Ok(json);
        }
        discarded += 1;
        let kind = json
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("<sin type>");
        if discarded <= 20 || discarded % 100 == 0 {
            eprintln!(
                "[rpc] {} descartando evento async type={} (#{} en total)",
                command, kind, discarded
            );
        }
        if discarded >= MAX_DISCARDED {
            return Err(format!(
                "RPC {}: respuesta nunca llegó ({} líneas descartadas)",
                command, discarded
            ));
        }
    }
}

#[derive(Debug, Clone)]
pub enum SseEvent {
    Token { content: String },
    ToolCall { name: String },
    ToolResult { name: String, status: String },
    Warning { message: String },
    Error { message: String },
    SessionStats { json: String },
    Done,
}

impl SseEvent {
    pub fn to_sse_string(&self) -> String {
        match self {
            SseEvent::Token { content } => {
                serde_json::json!({"type":"token","content":content}).to_string()
            }
            SseEvent::ToolCall { name } => {
                serde_json::json!({"type":"tool_call","name":name}).to_string()
            }
            SseEvent::ToolResult { name, status } => {
                serde_json::json!({"type":"tool_result","name":name,"status":status}).to_string()
            }
            SseEvent::Warning { message } => {
                serde_json::json!({"type":"warning","message":message}).to_string()
            }
            SseEvent::Error { message } => {
                serde_json::json!({"type":"error","message":message}).to_string()
            }
            SseEvent::SessionStats { json } => {
                serde_json::json!({"type":"session_stats","json":json}).to_string()
            }
            SseEvent::Done => {
                serde_json::json!({"type":"done"}).to_string()
            }
        }
    }
}

pub trait AgentBackend: Send {
    fn spawn(&mut self) -> Result<(), String>;
    fn send_message(&mut self, message: &str) -> Result<tokio::sync::mpsc::Receiver<SseEvent>, String>;
    fn shutdown(&mut self);

    fn restart(&mut self) -> Result<(), String> {
        self.shutdown();
        self.spawn()
    }

    fn get_session_stats(&self) -> Result<String, String> {
        Err("get_session_stats not supported by this backend".to_string())
    }
    fn get_state(&self) -> Result<String, String> {
        Err("get_state not supported by this backend".to_string())
    }
    fn new_session(&mut self) -> Result<(), String> {
        Err("new_session not supported by this backend".to_string())
    }

    fn set_agent_prompt(&mut self, _prompt: Option<String>) {}

    /// Devuelve el prompt pendiente sin consumirlo (para preservarlo al cambiar de backend).
    fn peek_agent_prompt(&self) -> Option<String> {
        None
    }

    fn set_model_rpc(&mut self, _provider: &str, _model_id: &str) -> Result<(), String> {
        Err("set_model_rpc not supported by this backend".to_string())
    }

    fn config(&self) -> &ChatConfig;
}

fn env_var_for_provider(provider: &str) -> &'static str {
    match provider {
        "opencode" | "opencode-go" => "OPENCODE_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "google" => "GEMINI_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        "groq" => "GROQ_API_KEY",
        "xai" => "XAI_API_KEY",
        _ => "OPENROUTER_API_KEY",
    }
}

pub struct PiAgentBackend {
    config: ChatConfig,
    child: Option<Child>,
    stdin: Option<Arc<Mutex<ChildStdin>>>,
    stdout: Option<Arc<Mutex<ChildStdout>>>,
    stderr: Option<Arc<Mutex<ChildStderr>>>,
    thinking_configured: bool,
    current_agent_prompt: Option<String>,
}

impl PiAgentBackend {
    pub fn new(config: ChatConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            stdout: None,
            stderr: None,
            thinking_configured: false,
            current_agent_prompt: None,
        }
    }

    pub fn set_agent_prompt(&mut self, prompt: Option<String>) {
        self.current_agent_prompt = prompt;
    }

    pub fn config(&self) -> &ChatConfig {
        &self.config
    }

    pub fn get_state(&self) -> Result<String, String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?;

        let resp = rpc_exchange(stdin, stdout_arc, "get_state", serde_json::json!({"type": "get_state"}))?;
        serde_json::to_string(&resp).map_err(|e| format!("JSON serialize: {}", e))
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?;

        let resp = rpc_exchange(stdin, stdout_arc, "get_session_stats", serde_json::json!({"type": "get_session_stats"}))?;
        serde_json::to_string(&resp).map_err(|e| format!("JSON serialize: {}", e))
    }

    pub fn new_session(&mut self) -> Result<(), String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?;

        let json = rpc_exchange(stdin, stdout_arc, "new_session", serde_json::json!({"type": "new_session"}))?;

        let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if success {
            Ok(())
        } else {
            let err = json.get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            Err(err.to_string())
        }
    }

    pub fn set_model_rpc(&mut self, provider: &str, model_id: &str) -> Result<(), String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?;

        let json = rpc_exchange(stdin, stdout_arc, "set_model", serde_json::json!({
            "type": "set_model",
            "provider": provider,
            "modelId": model_id,
        }))?;

        let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if success {
            self.config.model = model_id.to_string();
            self.config.provider = provider.to_string();
            Ok(())
        } else {
            let err = json.get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            Err(format!("set_model failed: {}", err))
        }
    }
}

impl AgentBackend for PiAgentBackend {
    fn get_session_stats(&self) -> Result<String, String> {
        PiAgentBackend::get_session_stats(self)
    }

    fn get_state(&self) -> Result<String, String> {
        PiAgentBackend::get_state(self)
    }

    fn new_session(&mut self) -> Result<(), String> {
        PiAgentBackend::new_session(self)
    }

    fn set_agent_prompt(&mut self, prompt: Option<String>) {
        PiAgentBackend::set_agent_prompt(self, prompt);
    }

    fn peek_agent_prompt(&self) -> Option<String> {
        self.current_agent_prompt.clone()
    }

    fn set_model_rpc(&mut self, provider: &str, model_id: &str) -> Result<(), String> {
        PiAgentBackend::set_model_rpc(self, provider, model_id)
    }

    fn config(&self) -> &ChatConfig {
        PiAgentBackend::config(self)
    }

    fn send_message(&mut self, message: &str) -> Result<tokio::sync::mpsc::Receiver<SseEvent>, String> {
        let (tx, rx) = tokio::sync::mpsc::channel(64);

        let stdin = self.stdin.as_ref()
            .ok_or_else(|| {
                let msg = "Pi not spawned (stdin is None)".to_string();
                eprintln!("[pi] send_message: {}", msg);
                msg
            })?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| {
                let msg = "Pi not spawned (stdout is None)".to_string();
                eprintln!("[pi] send_message: {}", msg);
                msg
            })?
            .clone();
        let stderr_arc = self.stderr.as_ref().map(Arc::clone);

        if let Some(level) = &self.config.thinking_level {
            if !self.thinking_configured {
                let pi_level = match level.as_str() {
                    "max" => "xhigh",
                    other => other,
                };

                // set_thinking_level (respuesta correlacionada por id; los eventos
                // async de pi/little-coder se descartan dentro de rpc_exchange)
                match rpc_exchange(stdin, &stdout_arc, "set_thinking_level", serde_json::json!({
                    "type": "set_thinking_level",
                    "level": pi_level,
                })) {
                    Ok(json) => {
                        let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                        if !success {
                            let err = json.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                            eprintln!("[pi] set_thinking_level FAILED: {} (level: {})", err, pi_level);
                        }
                    }
                    Err(e) => eprintln!("[pi] set_thinking_level RPC error: {}", e),
                }

                // get_state post-clamping (verificación; el resultado se loguea desde main.rs)
                if let Err(e) = rpc_exchange(stdin, &stdout_arc, "get_state", serde_json::json!({"type": "get_state"})) {
                    eprintln!("[pi] get_state (verificación thinking) error: {}", e);
                }

                self.thinking_configured = true;
            }
        }

        let final_message = if let Some(prompt) = self.current_agent_prompt.take() {
            format!("[System instructions]\n{}\n\n---\n\n{}", prompt, message)
        } else {
            message.to_string()
        };

        let request = serde_json::json!({
            "type": "prompt",
            "message": final_message,
        });

        let request_str = serde_json::to_string(&request).unwrap_or_default();

        {
            let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
            writeln!(stdin_lock, "{}", request_str)
                .map_err(|e| format!("Failed to write to pi stdin: {}", e))?;
            stdin_lock.flush()
                .map_err(|e| format!("Failed to flush pi stdin: {}", e))?;
        }

        // Thread for stderr: capture pi warnings/errors and forward them as SseEvent::Error
        if let Some(stderr_arc) = stderr_arc {
            let tx_err = tx.clone();
            thread::spawn(move || {
                let mut stderr_lock = match stderr_arc.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                let mut reader = BufReader::new(&mut *stderr_lock);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                let _ = tx_err.blocking_send(SseEvent::Warning {
                                    message: trimmed.to_string(),
                                });
                            }
                        }
                    }
                }
            });
        }

        let stdin_arc = stdin.clone();

        // Thread for stdout: parse JSON event stream
        thread::spawn(move || {
            let mut stdout_lock = match stdout_arc.lock() {
                Ok(g) => g,
                Err(_) => {
                    eprintln!("[pi] reader: stdout lock failed");
                    let _ = tx.blocking_send(SseEvent::Error {
                        message: "stdout lock failed".to_string(),
                    });
                    let _ = tx.blocking_send(SseEvent::Done);
                    return;
                }
            };

            let mut reader = BufReader::new(&mut *stdout_lock);
            let mut line = String::new();
            let mut agent_ended = false;

            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        break;
                    }
                    Err(e) => {
                        eprintln!("[pi] reader: read error: {}", e);
                        break;
                    }
                    Ok(n) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<serde_json::Value>(trimmed) {
                            Ok(json) => {
                                let event_type = json.get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                match event_type {
                                    "response" => {
                                        let success = json.get("success")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false);
                                        if !success {
                                            let err = json.get("error")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("Unknown error");
                                            let _ = tx.blocking_send(SseEvent::Error {
                                                message: err.to_string(),
                                            });
                                            break;
                                        }
                                    }
                                    "message_update" => {
                                        if let Some(ae) = json.get("assistantMessageEvent") {
                                            match ae.get("type").and_then(|v| v.as_str()) {
                                                Some("text_delta") => {
                                                    if let Some(delta) = ae.get("delta").and_then(|v| v.as_str()) {
                                                        let _ = tx.blocking_send(SseEvent::Token {
                                                            content: delta.to_string(),
                                                        });
                                                    }
                                                }
                                                Some("toolcall_end") => {
                                                    if let Some(tc) = ae.get("toolCall") {
                                                        let name = tc.get("name")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("tool");
                                                        let _ = tx.blocking_send(SseEvent::ToolCall {
                                                            name: name.to_string(),
                                                        });
                                                    }
                                                }
                                                Some("error") => {
                                                    let reason = ae.get("reason")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("unknown");
                                                    let _ = tx.blocking_send(SseEvent::Error {
                                                        message: format!("Stream error: {}", reason),
                                                    });
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    "tool_execution_start" => {
                                        let name = json.get("toolName")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("tool");
                                        let _ = tx.blocking_send(SseEvent::ToolCall {
                                            name: name.to_string(),
                                        });
                                    }
                                    "tool_execution_end" => {
                                        let name = json.get("toolName")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("tool");
                                        let is_err = json.get("isError")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false);
                                        let _ = tx.blocking_send(SseEvent::ToolResult {
                                            name: name.to_string(),
                                            status: if is_err { "error" } else { "ok" }.to_string(),
                                        });
                                    }
                                    "agent_end" => {
                                        agent_ended = true;
                                        break;
                                    }
                                    "error" => {
                                        let msg = json.get("error")
                                                                            .and_then(|v| v.as_str())
                                                                            .or_else(|| json.get("message").and_then(|v| v.as_str()))
                                                                            .unwrap_or("Unknown error");
                                        let _ = tx.blocking_send(SseEvent::Error {
                                            message: msg.to_string(),
                                        });
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            Err(_) => {
                                let _ = tx.blocking_send(SseEvent::Token {
                                    content: trimmed.to_string(),
                                });
                            }
                        }
                    }
                }
            }

            // Release stdout lock before doing RPC
            drop(reader);
            drop(stdout_lock);

            // After stream ends, fetch and send real session stats.
            // Correlación por id: si pi emite algún evento async antes de la
            // respuesta (extension_ui_request, widgets), se descarta dentro de
            // rpc_exchange en lugar de parsearse como stats.
            if agent_ended {
                match rpc_exchange(&stdin_arc, &stdout_arc, "get_session_stats", serde_json::json!({"type": "get_session_stats"})) {
                    Ok(json) => {
                        if let Ok(s) = serde_json::to_string(&json) {
                            let _ = tx.blocking_send(SseEvent::SessionStats { json: s });
                        }
                    }
                    Err(e) => eprintln!("[pi] get_session_stats post-stream error: {}", e),
                }
            }

            let _ = tx.blocking_send(SseEvent::Done);
        });

        Ok(rx)
    }

    fn spawn(&mut self) -> Result<(), String> {
        let binary = match self.config.flavor {
            BackendFlavor::Pi => self.config.pi_path.clone(),
            BackendFlavor::LittleCoder => self.config.lc_path.clone(),
        };

        let mut cmd = Command::new(&binary);
        cmd.args(["--mode", "rpc"])
            .arg("--provider").arg(&self.config.provider)
            .arg("--model").arg(&self.config.model);

        if matches!(self.config.flavor, BackendFlavor::LittleCoder) {
            // little-coder lanza pi con --no-extensions; este flag restaura la
            // discovery del ecosistema pi instalado en ~/.pi/agent (pi-opencode-provider,
            // pi-subagents, ...). --no-update-check evita que el launcher consulte el
            // registro npm y bloquee el arranque. Ambos flags los filtra el launcher
            // antes de pasárselos a pi.
            cmd.arg("--with-pi-extensions").arg("--no-update-check");
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Grupo de procesos propio: little-coder es un wrapper Node que spawnea a pi
        // como nieto; sin esto, un kill al wrapper huérfanaría a pi.
        #[cfg(unix)]
        cmd.process_group(0);

        // Inyectar TODOS los providers configurados: modelRuntime.getAvailable()
        // filtra por providers con credenciales, así que si solo inyectamos la key
        // del provider activo, un switch cruzado (Miku/groq → Teto/opencode) hace
        // que set_model falle con "Model not found" aunque el modelo exista.
        cmd.env(env_var_for_provider(&self.config.provider), &self.config.api_key);
        for m in crate::config::models::list() {
            let key = crate::config::keys::KeysStore::resolve(&m.api_key);
            if !key.is_empty() {
                cmd.env(env_var_for_provider(&m.provider), key);
            }
        }
        cmd.env("PI_CACHE_RETENTION", "long");

        // eprintln!("[pi] spawn: flavor={}, path={}, provider={}, model={}", self.config.flavor.as_str(), binary, self.config.provider, self.config.model);

        cmd.current_dir(crate::config::current_root::get());
        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("Failed to spawn {} agent: {}", self.config.flavor.as_str(), e);
            eprintln!("[pi] {}", msg);
            msg
        })?;

        // eprintln!("[pi] spawned with PID={}", child.id());

        let stdin = child.stdin.take()
            .ok_or_else(|| {
                let msg = "Failed to capture pi stdin".to_string();
                eprintln!("[pi] {}", msg);
                msg
            })?;
        let stdout = child.stdout.take()
            .ok_or_else(|| {
                let msg = "Failed to capture pi stdout".to_string();
                eprintln!("[pi] {}", msg);
                msg
            })?;
        let stderr = child.stderr.take()
            .ok_or_else(|| {
                let msg = "Failed to capture pi stderr".to_string();
                eprintln!("[pi] {}", msg);
                msg
            })?;

        // Pequeña pausa para detectar si pi muere inmediatamente (como rpc-client.ts)
        std::thread::sleep(std::time::Duration::from_millis(200));
        match child.try_wait() {
            Ok(Some(status)) => {
                let msg = format!("pi exited immediately with status={}", status);
                eprintln!("[pi] spawn: {}", msg);
                return Err(msg);
            }
            Ok(None) => {
                // eprintln!("[pi] spawn: pi is still running after 200ms, good");
            }
            Err(e) => {
                eprintln!("[pi] spawn: try_wait error: {}", e);
            }
        }

        self.stdin = Some(Arc::new(Mutex::new(stdin)));
        self.stdout = Some(Arc::new(Mutex::new(stdout)));
        self.stderr = Some(Arc::new(Mutex::new(stderr)));
        self.child = Some(child);
        Ok(())
    }

    fn shutdown(&mut self) {
        self.stdin = None;
        self.stdout = None;
        self.stderr = None;
        self.thinking_configured = false;
        if let Some(mut child) = self.child.take() {
            #[cfg(unix)]
            {
                // Con process_group(0), el pgid == pid del wrapper. SIGTERM al grupo
                // permite al launcher de little-coder reenviar la señal a pi y salir
                // limpio; SIGKILL directo dejaría a pi corriendo como huérfano.
                let pgid = child.id() as i32;
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;

                let _ = kill(Pid::from_raw(-pgid), Signal::SIGTERM);
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1500);
                while std::time::Instant::now() < deadline {
                    match child.try_wait() {
                        Ok(Some(_)) => return,
                        Ok(None) => thread::sleep(std::time::Duration::from_millis(50)),
                        Err(_) => break,
                    }
                }
                let _ = kill(Pid::from_raw(-pgid), Signal::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct NullBackend {
    config: ChatConfig,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            config: ChatConfig {
                provider: String::new(),
                model: String::new(),
                api_key: String::new(),
                pi_path: find_pi_path(),
                thinking_level: None,
                flavor: default_flavor(),
                lc_path: find_little_coder_path(),
            },
        }
    }
}

impl AgentBackend for NullBackend {
    fn spawn(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn new_session(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn send_message(&mut self, message: &str) -> Result<tokio::sync::mpsc::Receiver<SseEvent>, String> {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let msg = message.to_string();
        thread::spawn(move || {
            let _ = tx.blocking_send(SseEvent::Token {
                content: format!("[echo] {}\n\n(Conecta Pi Agent para respuestas reales)", msg),
            });
            let _ = tx.blocking_send(SseEvent::Done);
        });
        Ok(rx)
    }

    fn set_agent_prompt(&mut self, _prompt: Option<String>) {}

    fn config(&self) -> &ChatConfig {
        &self.config
    }

    fn shutdown(&mut self) {}
}
