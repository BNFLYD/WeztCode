pub mod wlroots;

pub use wlroots::WlrootsWindowManager;

pub trait WindowManager {
    /// Obtiene la geometría (x, y, width, height) de la ventana con el app_id especificado
    fn get_window_geometry(&self, app_id: &str) -> Option<WindowGeometry>;

    /// Suscribe a cambios de geometría en tiempo real
    fn subscribe_geometry_changes(&self, app_id: &str, callback: Box<dyn Fn(WindowGeometry) + Send>) -> Result<(), String>;

    /// Verifica si una ventana está enfocada
    fn is_window_focused(&self, app_id: &str) -> bool;

    /// Registra un callback para cuando cambia el estado de foco de la ventana
    /// El callback recibe (app_id, is_focused)
    fn on_focus_change(&self, app_id: &str, callback: Box<dyn Fn(bool) + Send>) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowGeometry {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self { x, y, width, height }
    }
}

/// Detecta el compositor actual y devuelve el WM apropiado
pub fn detect_window_manager() -> Option<Box<dyn WindowManager>> {
    // Por ahora solo soportamos wlroots
    // En el futuro podemos detectar el compositor via environment variables
    // o intentar conectar a diferentes protocols

    if let Ok(display) = std::env::var("WAYLAND_DISPLAY") {
        if !display.is_empty() {
            // Intentar wlroots
            return Some(Box::new(WlrootsWindowManager::new()));
        }
    }

    None
}
