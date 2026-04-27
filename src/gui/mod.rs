use crate::gui::protocol::wayland::wm::{WindowGeometry, WmEvent};
use std::sync::mpsc;

pub trait GuiPlatform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String>;
    fn set_geometry(&self, x: i32, y: i32, width: u32, height: u32);
    fn show(&self);
    fn hide(&self);
    fn is_available() -> bool where Self: Sized;

    /// Handle WM events by wiring them to GUI actions
    /// Default implementation - override if needed for specific platform
    fn handle_wm_events(&self, receiver: mpsc::Receiver<WmEvent>) {
        use gtk4::glib;

        glib::idle_add_local(move || {
            match receiver.try_recv() {
                Ok(event) => {
                    match event {
                        WmEvent::WindowFocused { .. } => {
                            println!("Window focused - showing overlay");
                        }
                        WmEvent::WindowUnfocused { .. } => {
                            println!("Window unfocused - hiding overlay");
                        }
                        WmEvent::GeometryChanged { geometry, .. } => {
                            println!("Geometry changed: {:?}", geometry);
                        }
                        _ => {}
                    }
                    glib::ControlFlow::Continue
                }
                Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    }
}

pub mod gtk4_linux;
pub mod protocol;

pub use gtk4_linux::Gtk4Platform;
