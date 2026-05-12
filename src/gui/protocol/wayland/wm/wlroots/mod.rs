pub mod foreign_toplevel;
pub mod sway_ipc;

use super::{WindowGeometry, WmEvent, WindowManager};
use std::sync::{Mutex, mpsc};
use std::thread;

pub struct WlrootsWindowManager {
    sender: Mutex<Option<mpsc::Sender<WmEvent>>>,
    capture_signal_receiver: Mutex<Option<mpsc::Receiver<()>>>,
}

impl WlrootsWindowManager {
    pub fn new() -> Self {
        Self {
            sender: Mutex::new(None),
            capture_signal_receiver: Mutex::new(None),
        }
    }
}

impl WindowManager for WlrootsWindowManager {
    fn event_receiver(&self) -> mpsc::Receiver<WmEvent> {
        let (tx, rx) = mpsc::channel();
        *self.sender.lock().unwrap() = Some(tx);
        rx
    }

    fn set_capture_signal(&self, signal_rx: mpsc::Receiver<()>) {
        // Store the receiver - it will be taken in start_monitoring
        // We use Option to allow taking (moving) the receiver later
        *self.capture_signal_receiver.lock().unwrap() = Some(signal_rx);
    }

    fn start_monitoring(&self, target_app_id: String, target_toplevel_id: Option<String>) -> Option<WindowGeometry> {
        let sender = self.sender.lock().unwrap().clone();
        let capture_signal_opt = self.capture_signal_receiver.lock().unwrap().take();

        // Create channel to receive initial geometry from the thread
        let (geometry_tx, geometry_rx) = mpsc::channel::<Option<WindowGeometry>>();

        thread::spawn(move || {
            if let Some(s) = sender {
                // Get the capture signal receiver, or create a dummy one if not set
                let capture_rx = capture_signal_opt.unwrap_or_else(|| {
                    // If no signal channel was set, create one and send immediately
                    let (tx, rx) = mpsc::channel();
                    let _ = tx.send(());
                    rx
                });

                // Start Sway IPC monitoring with capture signal channel
                if let Ok(client) = sway_ipc::SwayIpcClient::new() {
                    let initial_geometry = client.subscribe_window_events(target_app_id, target_toplevel_id, s, capture_rx)
                        .ok()
                        .flatten();
                    // Send initial geometry back to main thread
                    let _ = geometry_tx.send(initial_geometry);
                } else {
                    let _ = geometry_tx.send(None);
                }
            } else {
                let _ = geometry_tx.send(None);
            }
        });

        // Wait for initial geometry (with timeout-like behavior using recv)
        geometry_rx.recv().ok().flatten()
    }

    fn get_window_geometry(&self, _app_id: &str) -> Option<WindowGeometry> {
        // TODO: Integrate sway_ipc for geometry
        None
    }

    fn is_window_focused(&self, _app_id: &str) -> bool {
        // TODO: Implement via foreign_toplevel
        false
    }

    fn wm_name(&self) -> &'static str {
        "wlroots (Wayland)"
    }
}
