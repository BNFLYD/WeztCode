pub mod wlroots;
pub mod detection;

pub use wlroots::WlrootsWindowManager;

use std::sync::mpsc;

/// Unified event type for ALL window manager implementations
/// Events are sent from WM threads to the GUI via channel
#[derive(Debug, Clone)]
pub enum WmEvent {
    /// Window gained focus
    WindowFocused { app_id: String },
    /// Window lost focus
    WindowUnfocused { app_id: String },
    /// Window geometry changed
    GeometryChanged { app_id: String, geometry: WindowGeometry },
    /// New window created
    WindowCreated { app_id: String },
    /// Window destroyed/closed
    WindowDestroyed { app_id: String },
}

/// Platform-agnostic window manager trait
/// All implementations must be Send + 'static for thread safety
pub trait WindowManager: Send + 'static {
    /// Returns a receiver for all WM events
    /// Events are sent from internal threads to this channel
    fn event_receiver(&self) -> mpsc::Receiver<WmEvent>;

    /// Start monitoring target window in background thread(s)
    /// Events will be sent to the receiver returned by event_receiver()
    /// target_toplevel_id is optional - if provided, it helps identify the exact window instance
    fn start_monitoring(&self, target_app_id: String, target_toplevel_id: Option<String>);

    /// Set signal receiver to trigger toplevel_id capture
    /// This is called after the target window is known to be ready
    fn set_capture_signal(&self, _signal_rx: mpsc::Receiver<()>);

    /// Synchronous query - current geometry
    fn get_window_geometry(&self, app_id: &str) -> Option<WindowGeometry>;

    /// Synchronous query - current focus state
    fn is_window_focused(&self, app_id: &str) -> bool;

    /// Get WM name for debugging/logging
    fn wm_name(&self) -> &'static str;
}

/// Hierarchical detection - returns appropriate implementation for current platform
pub fn detect_window_manager() -> Option<Box<dyn WindowManager>> {
    use detection::*;

    match detect_display_server() {
        DisplayServer::Wayland => {
            match detect_wayland_compositor() {
                WaylandCompositor::Wlroots => {
                    Some(Box::new(wlroots::WlrootsWindowManager::new()))
                }
                WaylandCompositor::KWin => {
                    eprintln!("KWin Wayland support not yet implemented");
                    None
                }
                WaylandCompositor::Mutter => {
                    eprintln!("Mutter Wayland support not yet implemented");
                    None
                }
                WaylandCompositor::Cosmic => {
                    eprintln!("Cosmic support not yet implemented");
                    None
                }
                WaylandCompositor::Weston => {
                    eprintln!("Weston support not yet implemented");
                    None
                }
                WaylandCompositor::Unknown(_) => {
                    eprintln!("Unknown Wayland compositor");
                    None
                }
            }
        }

        DisplayServer::X11 => {
            match detect_x11_wm() {
                X11WindowManager::KWin => {
                    eprintln!("KWin X11 support not yet implemented");
                    None
                }
                X11WindowManager::Mutter => {
                    eprintln!("Mutter X11 support not yet implemented");
                    None
                }
                X11WindowManager::Metacity => {
                    eprintln!("Metacity support not yet implemented");
                    None
                }
                X11WindowManager::Xfwm => {
                    eprintln!("XFWM support not yet implemented");
                    None
                }
                X11WindowManager::Marco => {
                    eprintln!("Marco support not yet implemented");
                    None
                }
                _ => {
                    eprintln!("X11 EWMH support not yet implemented");
                    None
                }
            }
        }

        DisplayServer::Unknown => {
            eprintln!("Unknown display server");
            None
        }
    }
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
