mod backend;

pub use backend::*;

use std::thread;

pub struct ChatService {
    backend: Box<dyn AgentBackend>,
}

impl ChatService {
    pub fn new(mut backend: Box<dyn AgentBackend>) -> Self {
        if let Err(e) = sync_pi_model_overrides() {
            eprintln!("[chat] Failed to sync pi model overrides: {}", e);
        }
        if let Err(e) = backend.spawn() {
            eprintln!("[chat] Failed to spawn agent backend: {}", e);
        }
        Self { backend }
    }

    pub fn switch_backend(&mut self, new_backend: Box<dyn AgentBackend>) -> Result<(), String> {
        let mut backend = new_backend;
        if let Err(e) = sync_pi_model_overrides() {
            eprintln!("[chat] Failed to sync pi model overrides: {}", e);
        }
        backend.spawn()?;
        self.backend = backend;
        Ok(())
    }

    pub fn switch_agent(&mut self, entry: &crate::config::sub_agents::SubAgentEntry) {
        let prompt = if entry.system_prompt.is_empty() {
            None
        } else {
            Some(entry.system_prompt.clone())
        };
        self.backend.set_agent_prompt(prompt);
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        self.backend.get_session_stats()
    }

    pub fn get_state(&self) -> Result<String, String> {
        self.backend.get_state()
    }

    pub fn new_session(&mut self) -> Result<(), String> {
        self.backend.new_session()
    }

    pub fn restart_backend(&mut self) -> Result<(), String> {
        self.backend.restart()
    }

    pub fn send_message_stream(&mut self, message: &str) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        let mut rx = self.backend.send_message(message)?;
        let (tx, out_rx) = tokio::sync::mpsc::channel::<String>(64);

        thread::spawn(move || {
            while let Some(event) = rx.blocking_recv() {
                let sse_str = event.to_sse_string();
                if tx.blocking_send(sse_str).is_err() { break; }
                if matches!(event, SseEvent::Done) { break; }
            }
        });

        Ok(out_rx)
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        self.backend.shutdown();
    }
}
