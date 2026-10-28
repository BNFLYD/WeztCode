//! Ventana viewer de puertos (sección 表示): canal thread-safe API → main loop
//! de GTK, estado global y layout del menú para anclar la ventana "al lado".
//!
//! GTK no es thread-safe: las ventanas se crean/destruyen en el main loop. La
//! API (handlers axum, cualquier thread) manda comandos por un glib MainContext
//! channel y el lado GTK los ejecuta. El layout del menú (márgenes + ancho) se
//! guarda en un static para que el viewer pueda anclarse inmediatamente a la
//! izquierda de la interfaz.

use once_cell::sync::Lazy;
use std::sync::Mutex;

/// Comandos que la API le manda al main loop de GTK.
#[derive(Debug, Clone)]
pub enum ViewerCmd {
    /// Abrir el viewer en ese puerto (reemplaza el contenido si ya está abierto).
    Open { port: u16 },
    /// Cerrar la ventana.
    Close,
}

/// Sender al main loop de GTK (tokio unbounded channel: el recv().await es
/// un Future puro que glib maneja con spawn_local — event-driven, sin polling).
/// Se setea en Gtk4Platform::create_overlay; None hasta que GTK bootea.
pub static VIEWER_TX: Lazy<Mutex<Option<tokio::sync::mpsc::UnboundedSender<ViewerCmd>>>> =
    Lazy::new(|| Mutex::new(None));

/// Puerto que el viewer está mostrando (None = ventana cerrada).
/// Lo escribe el lado GTK; la API solo lo lee para /ports/status.
pub static VIEWER_STATUS: Lazy<Mutex<Option<u16>>> = Lazy::new(|| Mutex::new(None));

/// Layout del menú (márgenes + ancho), para posicionar la ventana viewer
/// exactamente al lado de la interfaz.
#[derive(Debug, Clone, Copy)]
pub struct MenuLayout {
    pub top: i32,
    pub bottom: i32,
    pub right: i32,
    pub width: i32,
    /// Margen izquierdo de la terminal (Phase 3): el viewer ancla Left acá
    /// y llena el rectángulo libre al lado del menú.
    pub left: i32,
}

impl Default for MenuLayout {
    fn default() -> Self {
        Self { top: 1, bottom: 1, right: 0, width: 350, left: 0 }
    }
}

pub static MENU_LAYOUT: Lazy<Mutex<MenuLayout>> = Lazy::new(|| Mutex::new(MenuLayout::default()));

pub fn set_menu_layout(layout: MenuLayout) {
    *MENU_LAYOUT.lock().unwrap() = layout;
}

pub fn menu_layout() -> MenuLayout {
    *MENU_LAYOUT.lock().unwrap()
}

/// Crea el channel y registra el sender global. Devuelve el receiver para
/// que el main loop de GTK lo consuma con spawn_local (glib maneja el Future).
pub fn setup_channel() -> tokio::sync::mpsc::UnboundedReceiver<ViewerCmd> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<ViewerCmd>();
    *VIEWER_TX.lock().unwrap() = Some(tx);
    rx
}

pub fn set_status(port: Option<u16>) {
    *VIEWER_STATUS.lock().unwrap() = port;
}

pub fn status() -> Option<u16> {
    *VIEWER_STATUS.lock().unwrap()
}

// ---- lado API (cualquier thread) ----

pub fn open_port(port: u16) -> Result<(), String> {
    let guard = VIEWER_TX.lock().map_err(|e| format!("Lock: {}", e))?;
    let tx = guard.as_ref().ok_or("Viewer no disponible (GTK no inició)")?;
    tx.send(ViewerCmd::Open { port }).map_err(|_| "Canal de GTK cerrado".to_string())
}

pub fn close_port() -> Result<(), String> {
    let guard = VIEWER_TX.lock().map_err(|e| format!("Lock: {}", e))?;
    let tx = guard.as_ref().ok_or("Viewer no disponible (GTK no inició)")?;
    tx.send(ViewerCmd::Close).map_err(|_| "Canal de GTK cerrado".to_string())
}
