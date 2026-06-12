use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub struct ChatConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub pi_path: String,
    pub thinking_level: Option<String>,
    pub tools: Option<String>,
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
                tools: None,
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
            tools: None,
        }
    }

    pub fn from_sub_agent(entry: &crate::config::sub_agents::SubAgentEntry) -> Self {
        let provider = entry.model.split('/').next().unwrap_or("openrouter").to_string();
        let api_key = crate::config::keys::KeysStore::resolve(
            &format!("KEYS.{}", provider.to_uppercase())
        );
        Self {
            provider,
            model: entry.model.clone(),
            api_key,
            pi_path: find_pi_path(),
            thinking_level: None,
            tools: entry.tools.clone(),
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
            tools: None,
        }
    }
}

fn find_pi_path() -> String {
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
    fn get_session_stats(&self) -> Result<String, String> {
        Err("get_session_stats not supported by this backend".to_string())
    }
    fn get_state(&self) -> Result<String, String> {
        Err("get_state not supported by this backend".to_string())
    }
    fn new_session(&mut self) -> Result<(), String> {
        Err("new_session not supported by this backend".to_string())
    }
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
        }
    }

    pub fn config(&self) -> &ChatConfig {
        &self.config
    }

    pub fn get_state(&self) -> Result<String, String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?
            .clone();

        let msg = serde_json::json!({"type": "get_state"});
        let msg_str = serde_json::to_string(&msg).map_err(|e| format!("JSON serialize: {}", e))?;

        {
            let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
            writeln!(stdin_lock, "{}", msg_str)
                .map_err(|e| format!("Failed to write get_state: {}", e))?;
            stdin_lock.flush()
                .map_err(|e| format!("Failed to flush get_state: {}", e))?;
        }

        let mut stdout_lock = stdout_arc.lock().map_err(|e| format!("stdout lock: {}", e))?;
        let mut bytes = Vec::new();
        let mut buf = [0u8; 1];
        loop {
            match stdout_lock.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if buf[0] == b'\n' { break; }
                    bytes.push(buf[0]);
                }
                Err(e) => return Err(format!("stdout read error: {}", e)),
            }
        }

        String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {}", e))
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?
            .clone();

        let msg = serde_json::json!({"type": "get_session_stats"});
        let msg_str = serde_json::to_string(&msg).map_err(|e| format!("JSON serialize: {}", e))?;

        {
            let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
            writeln!(stdin_lock, "{}", msg_str)
                .map_err(|e| format!("Failed to write get_session_stats: {}", e))?;
            stdin_lock.flush()
                .map_err(|e| format!("Failed to flush get_session_stats: {}", e))?;
        }

        let mut stdout_lock = stdout_arc.lock().map_err(|e| format!("stdout lock: {}", e))?;
        let mut bytes = Vec::new();
        let mut buf = [0u8; 1];
        loop {
            match stdout_lock.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if buf[0] == b'\n' { break; }
                    bytes.push(buf[0]);
                }
                Err(e) => return Err(format!("stdout read error: {}", e)),
            }
        }

        String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {}", e))
    }

    pub fn new_session(&mut self) -> Result<(), String> {
        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned (stdin is None)".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned (stdout is None)".to_string())?
            .clone();

        let msg = serde_json::json!({"type": "new_session"});
        let msg_str = serde_json::to_string(&msg).map_err(|e| format!("JSON serialize: {}", e))?;

        {
            let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
            writeln!(stdin_lock, "{}", msg_str)
                .map_err(|e| format!("Failed to write new_session: {}", e))?;
            stdin_lock.flush()
                .map_err(|e| format!("Failed to flush new_session: {}", e))?;
        }

        let mut stdout_lock = stdout_arc.lock().map_err(|e| format!("stdout lock: {}", e))?;
        let mut bytes = Vec::new();
        let mut buf = [0u8; 1];
        loop {
            match stdout_lock.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    if buf[0] == b'\n' { break; }
                    bytes.push(buf[0]);
                }
                Err(e) => return Err(format!("stdout read error: {}", e)),
            }
        }

        let response = String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {}", e))?;
        let json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| format!("JSON parse error: {}", e))?;

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

                // --- Send set_thinking_level RPC ---
                let set_msg = serde_json::json!({
                    "type": "set_thinking_level",
                    "level": pi_level,
                });
                let set_str = serde_json::to_string(&set_msg).unwrap_or_default();

                {
                    let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
                    writeln!(stdin_lock, "{}", set_str)
                        .map_err(|e| format!("Failed to write set_thinking_level: {}", e))?;
                    stdin_lock.flush()
                        .map_err(|e| format!("Failed to flush set_thinking_level: {}", e))?;
                }

                // --- Read set_thinking_level response synchronously ---
                {
                    let mut stdout_lock = stdout_arc.lock().map_err(|e| format!("stdout lock: {}", e))?;
                    let mut bytes = Vec::new();
                    let mut buf = [0u8; 1];
                    loop {
                        match stdout_lock.read(&mut buf) {
                            Ok(0) => break,
                            Ok(_) => {
                                if buf[0] == b'\n' { break; }
                                bytes.push(buf[0]);
                            }
                            Err(e) => {
                                // eprintln!("[pi] error reading set_thinking_level response: {}", e);
                                break;
                            }
                        }
                    }
                    if !bytes.is_empty() {
                        let response = String::from_utf8_lossy(&bytes);
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                            let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
                            if success {
                                // eprintln!("[pi] set_thinking_level confirmed: {} ✓", pi_level);
                            } else {
                                let err = json.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
                                // eprintln!("[pi] set_thinking_level FAILED: {} (level: {})", err, pi_level);
                            }
                        } else {
                            // eprintln!("[pi] set_thinking_level response (unparsed): {}", response);
                        }
                    }
                }

                // --- Send get_state RPC to verify actual level post-clamping ---
                let get_state_msg = serde_json::json!({"type": "get_state"});
                let get_state_str = serde_json::to_string(&get_state_msg).unwrap_or_default();

                {
                    let mut stdin_lock = stdin.lock().map_err(|e| format!("stdin lock: {}", e))?;
                    writeln!(stdin_lock, "{}", get_state_str)
                        .map_err(|e| format!("Failed to write get_state: {}", e))?;
                    stdin_lock.flush()
                        .map_err(|e| format!("Failed to flush get_state: {}", e))?;
                }

                // --- Read get_state response ---
                {
                    let mut stdout_lock = stdout_arc.lock().map_err(|e| format!("stdout lock: {}", e))?;
                    let mut bytes = Vec::new();
                    let mut buf = [0u8; 1];
                    loop {
                        match stdout_lock.read(&mut buf) {
                            Ok(0) => break,
                            Ok(_) => {
                                if buf[0] == b'\n' { break; }
                                bytes.push(buf[0]);
                            }
                            Err(e) => {
                                // eprintln!("[pi] error reading get_state response: {}", e);
                                break;
                            }
                        }
                    }
                    if !bytes.is_empty() {
                        // get_state verification (logged via main.rs after each message)
                    }
                }

                self.thinking_configured = true;
                // eprintln!("[pi] thinking_level configured successfully");
            }
        }

        let request = serde_json::json!({
            "type": "prompt",
            "message": message,
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

            // After stream ends, fetch and send real session stats
            if agent_ended {
                let msg = serde_json::json!({"type": "get_session_stats"});
                if let Ok(s) = serde_json::to_string(&msg) {
                    if let Ok(lock) = stdin_arc.lock() {
                        let _ = writeln!(&*lock, "{}", s);
                        let _ = (&*lock).flush();
                    }
                }

                if let Ok(mut stdout_lock) = stdout_arc.lock() {
                    let mut bytes = Vec::new();
                    let mut buf = [0u8; 1];
                    loop {
                        match stdout_lock.read(&mut buf) {
                            Ok(0) => break,
                            Ok(_) => {
                                if buf[0] == b'\n' { break; }
                                bytes.push(buf[0]);
                            }
                            Err(_) => break,
                        }
                    }
                    if !bytes.is_empty() {
                        if let Ok(s) = String::from_utf8(bytes) {
                            let _ = tx.blocking_send(SseEvent::SessionStats { json: s });
                        }
                    }
                }
            }

            let _ = tx.blocking_send(SseEvent::Done);
        });

        Ok(rx)
    }

    fn spawn(&mut self) -> Result<(), String> {
        let mut cmd = Command::new(&self.config.pi_path);
        cmd.args(["--mode", "rpc"])
            .arg("--provider").arg(&self.config.provider)
            .arg("--model").arg(&self.config.model);

        if let Some(tools) = &self.config.tools {
            cmd.arg("--tools").arg(tools);
        }

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let env_key = env_var_for_provider(&self.config.provider);
        cmd.env(env_key, &self.config.api_key);

        // eprintln!("[pi] spawn: path={}, provider={}, model={}", self.config.pi_path, self.config.provider, self.config.model);

        cmd.current_dir(crate::config::current_root::get());
        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("Failed to spawn pi: {}", e);
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
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct NullBackend;

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

    fn shutdown(&mut self) {}
}
