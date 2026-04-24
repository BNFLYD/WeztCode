use crate::gui::GuiPlatform;
use crate::wm::WindowGeometry;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use webkit6::prelude::*;
use webkit6::WebView;
use std::cell::RefCell;
use std::rc::Rc;

pub trait GuiPlatform {
    fn create_overlay(&self, url: &str, term_geometry: Option<WindowGeometry>) -> Result<(), String>;
    fn set_geometry(&self, x: i32, y: i32, width: u32, height: u32);
    fn show(&self);
    fn hide(&self);
    fn is_available() -> bool where Self: Sized;
}

pub mod gtk4_linux;

pub use gtk4_linux::Gtk4Platform;
