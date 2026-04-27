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
                // Start foreign_toplevel monitoring
                foreign_toplevel::start_focus_monitor(target_app_id.clone(), Box::new(move |focused| {
                    let event = if focused {
                        WmEvent::WindowFocused { app_id: target_app_id.clone() }
                    } else {
                        WmEvent::WindowUnfocused { app_id: target_app_id.clone() }
                    };
                    let _ = s.send(event);
                }));
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
