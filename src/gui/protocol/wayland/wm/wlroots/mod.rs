pub mod foreign_toplevel;
pub mod sway_ipc;

use super::{WindowGeometry, WmEvent, WindowManager};
use std::sync::{Mutex, mpsc};
use std::thread;

pub struct WlrootsWindowManager {
    sender: Mutex<Option<mpsc::Sender<WmEvent>>>,
}

impl WlrootsWindowManager {
    pub fn new() -> Self {
        Self {
            sender: Mutex::new(None),
        }
    }
}

impl WindowManager for WlrootsWindowManager {
    fn event_receiver(&self) -> mpsc::Receiver<WmEvent> {
        let (tx, rx) = mpsc::channel();
        *self.sender.lock().unwrap() = Some(tx);
        rx
    }

    fn start_monitoring(&self, target_app_id: String) {
        let sender = self.sender.lock().unwrap().clone();

        thread::spawn(move || {
            if let Some(s) = sender {
                // Start Sway IPC monitoring for focus and geometry events
                if let Ok(client) = sway_ipc::SwayIpcClient::new() {
                    let _ = client.subscribe_window_events(target_app_id, s);
                }
            }
        });
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
