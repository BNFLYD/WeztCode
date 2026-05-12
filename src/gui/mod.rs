use crate::gui::protocol::wayland::wm::{WindowGeometry, WmEvent};
use std::sync::mpsc;

pub trait GuiPlatform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String>;
    fn set_geometry(&self, x: i32, y: i32, width: u32, height: u32);
    fn show(&self);
    fn hide(&self);
    fn is_available() -> bool where Self: Sized;

    /// Handle WM events by wiring them to GUI actions
    /// Handle WM events - platform-specific implementations should override this
    fn handle_wm_events(&self, _receiver: mpsc::Receiver<WmEvent>) {
        // Default implementation does nothing - platforms must implement
        eprintln!("Warning: handle_wm_events not implemented for this platform");
    }
}

pub mod gtk4_linux;
pub mod protocol;

pub use gtk4_linux::Gtk4Platform;
