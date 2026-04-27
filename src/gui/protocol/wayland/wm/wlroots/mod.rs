pub mod foreign_toplevel;
pub mod sway_ipc;

use super::WindowGeometry;
use std::sync::{Arc, Mutex};

pub struct WlrootsWindowManager {
    // TODO: Agregar campos necesarios
}

impl WlrootsWindowManager {
    pub fn new() -> Self {
        Self {}
    }
}

impl super::WindowManager for WlrootsWindowManager {
    fn get_window_geometry(&self, _app_id: &str) -> Option<WindowGeometry> {
        // TODO: Integrar foreign_toplevel + sway_ipc
        None
    }

    fn subscribe_geometry_changes(&self, _app_id: &str, _callback: Box<dyn Fn(WindowGeometry) + Send>) -> Result<(), String> {
        // TODO: Implementar
        Err("Not implemented".to_string())
    }

    fn is_window_focused(&self, _app_id: &str) -> bool {
        // TODO: Implementar via foreign_toplevel
        false
    }

    fn on_focus_change(&self, app_id: &str, callback: Box<dyn Fn(bool) + Send>) -> Result<(), String> {
        foreign_toplevel::start_focus_monitor(app_id.to_string(), callback)
    }
}
