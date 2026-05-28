use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub struct ChatConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub pi_path: String,
}

impl ChatConfig {
    pub fn from_props() -> Self {
        let props = crate::config::props::UserProps::load();
        Self {
            provider: props.get("llm_provider").unwrap_or("openrouter").to_string(),
            model: props.get("llm_model").unwrap_or("openrouter/anthropic/claude-sonnet-4").to_string(),
            api_key: props.get_resolved("llm_api_key").unwrap_or_default(),
            pi_path: props.get("pi_path").unwrap_or("pi").to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SseEvent {
    Token { content: String },
    ToolCall { name: String },
    ToolResult { name: String, status: String },
    Error { message: String },
    Done,
}

impl SseEvent {
    pub fn to_sse_string(&self) -> String {
        match self {
            SseEvent::Token { content } => {
                let json = serde_json::json!({"type":"token","content":content});
                format!("data: {}\n\n", json)
            }
            SseEvent::ToolCall { name } => {
                let json = serde_json::json!({"type":"tool_call","name":name});
                format!("data: {}\n\n", json)
            }
            SseEvent::ToolResult { name, status } => {
                let json = serde_json::json!({"type":"tool_result","name":name,"status":status});
                format!("data: {}\n\n", json)
            }
            SseEvent::Error { message } => {
                let json = serde_json::json!({"type":"error","message":message});
                format!("data: {}\n\n", json)
            }
            SseEvent::Done => {
                format!("data: {}\n\n", serde_json::json!({"type":"done"}))
            }
        }
    }
}

pub trait AgentBackend: Send {
    fn spawn(&mut self) -> Result<(), String>;
    fn send_message(&mut self, message: &str) -> Result<mpsc::Receiver<SseEvent>, String>;
    fn shutdown(&mut self);
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
    stdin: Option<Mutex<ChildStdin>>,
    stdout: Option<Arc<Mutex<ChildStdout>>>,
}

impl PiAgentBackend {
    pub fn new(config: ChatConfig) -> Self {
        Self {
            config,
            child: None,
            stdin: None,
            stdout: None,
        }
    }
}

impl AgentBackend for PiAgentBackend {
    fn spawn(&mut self) -> Result<(), String> {
        let mut cmd = Command::new(&self.config.pi_path);
        cmd.args(["--mode", "rpc", "--no-session"])
            .arg("--provider").arg(&self.config.provider)
            .arg("--model").arg(&self.config.model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        let env_key = env_var_for_provider(&self.config.provider);
        cmd.env(env_key, &self.config.api_key);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn pi: {}", e))?;

        let stdin = child.stdin.take()
            .ok_or_else(|| "Failed to capture pi stdin".to_string())?;
        let stdout = child.stdout.take()
            .ok_or_else(|| "Failed to capture pi stdout".to_string())?;

        self.stdin = Some(Mutex::new(stdin));
        self.stdout = Some(Arc::new(Mutex::new(stdout)));
        self.child = Some(child);
        Ok(())
    }

    fn send_message(&mut self, message: &str) -> Result<mpsc::Receiver<SseEvent>, String> {
        let (tx, rx) = mpsc::channel();

        let stdin = self.stdin.as_ref()
            .ok_or_else(|| "Pi not spawned".to_string())?;
        let stdout_arc = self.stdout.as_ref()
            .ok_or_else(|| "Pi not spawned".to_string())?
            .clone();

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

        thread::spawn(move || {
            let mut stdout_lock = match stdout_arc.lock() {
                Ok(g) => g,
                Err(_) => {
                    let _ = tx.send(SseEvent::Error {
                        message: "stdout lock failed".to_string(),
                    });
                    let _ = tx.send(SseEvent::Done);
                    return;
                }
            };

            let mut reader = BufReader::new(&mut *stdout_lock);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        let _ = tx.send(SseEvent::Done);
                        break;
                    }
                    Err(_) => {
                        let _ = tx.send(SseEvent::Done);
                        break;
                    }
                    Ok(_) => {
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
                                            let _ = tx.send(SseEvent::Error {
                                                message: err.to_string(),
                                            });
                                            let _ = tx.send(SseEvent::Done);
                                            break;
                                        }
                                    }
                                    "message_update" => {
                                        if let Some(ae) = json.get("assistantMessageEvent") {
                                            match ae.get("type").and_then(|v| v.as_str()) {
                                                Some("text_delta") => {
                                                    if let Some(delta) = ae.get("delta").and_then(|v| v.as_str()) {
                                                        let _ = tx.send(SseEvent::Token {
                                                            content: delta.to_string(),
                                                        });
                                                    }
                                                }
                                                Some("toolcall_end") => {
                                                    if let Some(tc) = ae.get("toolCall") {
                                                        let name = tc.get("name")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("tool");
                                                        let _ = tx.send(SseEvent::ToolCall {
                                                            name: name.to_string(),
                                                        });
                                                    }
                                                }
                                                Some("error") => {
                                                    let reason = ae.get("reason")
                                                                .and_then(|v| v.as_str())
                                                                .unwrap_or("unknown");
                                                    let _ = tx.send(SseEvent::Error {
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
                                        let _ = tx.send(SseEvent::ToolCall {
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
                                        let _ = tx.send(SseEvent::ToolResult {
                                            name: name.to_string(),
                                            status: if is_err { "error" } else { "ok" }.to_string(),
                                        });
                                    }
                                    "agent_end" => {
                                        let _ = tx.send(SseEvent::Done);
                                        break;
                                    }
                                    "error" => {
                                        let msg = json.get("error")
                                                            .and_then(|v| v.as_str())
                                                            .or_else(|| json.get("message").and_then(|v| v.as_str()))
                                                            .unwrap_or("Unknown error");
                                        let _ = tx.send(SseEvent::Error {
                                            message: msg.to_string(),
                                        });
                                        let _ = tx.send(SseEvent::Done);
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            Err(_) => {
                                // Non-JSON line (shouldn't happen in RPC mode)
                                let _ = tx.send(SseEvent::Token {
                                    content: trimmed.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    fn shutdown(&mut self) {
        self.stdin = None;
        self.stdout = None;
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct ChannelReader {
    rx: mpsc::Receiver<Vec<u8>>,
    buffer: Vec<u8>,
    pos: usize,
}

impl ChannelReader {
    pub fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            rx,
            buffer: Vec::new(),
            pos: 0,
        }
    }
}

impl Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.buffer.len() {
            match self.rx.recv() {
                Ok(chunk) => {
                    self.buffer = chunk;
                    self.pos = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let n = std::cmp::min(buf.len(), self.buffer.len() - self.pos);
        buf[..n].copy_from_slice(&self.buffer[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

pub struct NullBackend;

impl AgentBackend for NullBackend {
    fn spawn(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn send_message(&mut self, message: &str) -> Result<mpsc::Receiver<SseEvent>, String> {
        let (tx, rx) = mpsc::channel();
        let msg = message.to_string();
        thread::spawn(move || {
            let _ = tx.send(SseEvent::Token {
                content: format!("[echo] {}\n\n(Conecta Pi Agent para respuestas reales)", msg),
            });
            let _ = tx.send(SseEvent::Done);
        });
        Ok(rx)
    }

    fn shutdown(&mut self) {}
}
