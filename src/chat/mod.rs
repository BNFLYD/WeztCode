mod backend;

pub use backend::*;

use std::sync::mpsc;
use std::thread;

pub struct ChatService {
    backend: Box<dyn AgentBackend>,
}

impl ChatService {
    pub fn new(mut backend: Box<dyn AgentBackend>) -> Self {
        if let Err(e) = backend.spawn() {
            eprintln!("[chat] Failed to spawn agent backend: {}", e);
        }
        Self { backend }
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
