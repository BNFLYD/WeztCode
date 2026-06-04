mod backend;

pub use backend::*;

use std::sync::mpsc;
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

    pub fn get_session_stats(&self) -> Result<String, String> {
        self.backend.get_session_stats()
    }

    pub fn get_state(&self) -> Result<String, String> {
        self.backend.get_state()
    }

    pub fn new_session(&mut self) -> Result<(), String> {
        self.backend.new_session()
    }

    pub fn send_message_stream(&mut self, message: &str) -> Result<ChannelReader, String> {
        let rx = self.backend.send_message(message)?;

        let (byte_tx, byte_rx) = mpsc::channel();

        thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(event) => {
                        let sse = event.to_sse_string();
                        let bytes = sse.into_bytes();
                        if byte_tx.send(bytes).is_err() {
                            break;
                        }
                        if matches!(event, SseEvent::Done) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(ChannelReader::new(byte_rx))
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        self.backend.shutdown();
    }
}
