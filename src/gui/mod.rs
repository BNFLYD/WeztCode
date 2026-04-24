use crate::gui::protocol::wayland::wm::WindowGeometry;

pub trait GuiPlatform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String>;
    fn set_geometry(&self, x: i32, y: i32, width: u32, height: u32);
    fn show(&self);
    fn hide(&self);
    fn is_available() -> bool where Self: Sized;
}

pub mod gtk4_linux;
pub mod protocol;

pub use gtk4_linux::Gtk4Platform;
