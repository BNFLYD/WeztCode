//! Window Manager Detection System
//!
//! Hierarchical detection of display server, compositor, and window manager.
//! Supports Wayland (wlroots, KWin, Mutter, Cosmic), X11 (DEs and WMs), Windows, and macOS.

use std::env;

/// Display server type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    Wayland,
    X11,
    Unknown,
}

/// Wayland compositor identification
#[derive(Debug, Clone, PartialEq)]
pub enum WaylandCompositor {
    Wlroots,
    KWin,
    Mutter,
    Cosmic,
    Weston,
    Unknown(String),
}

/// X11 window manager identification
#[derive(Debug, Clone, PartialEq)]
pub enum X11WindowManager {
    // Desktop Environments
    KWin,
    Mutter,
    Metacity,
    Xfwm,
    Marco,
    // Standalone WMs
    I3,
    Bspwm,
    Openbox,
    Awesome,
    Dwm,
    Xmonad,
    // Generic fallback
    GenericEWMH(String),
}

/// Detect current display server (Wayland or X11)
pub fn detect_display_server() -> DisplayServer {
    if env::var("WAYLAND_DISPLAY").is_ok() {
        DisplayServer::Wayland
    } else if env::var("DISPLAY").is_ok() {
        DisplayServer::X11
    } else {
        DisplayServer::Unknown
    }
}

/// Detect Wayland compositor
pub fn detect_wayland_compositor() -> WaylandCompositor {
    // Check XDG_CURRENT_DESKTOP for DE hint
    if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
        match desktop.as_str() {
            "KDE" => return WaylandCompositor::KWin,
            "GNOME" => return WaylandCompositor::Mutter,
            "COSMIC" => return WaylandCompositor::Cosmic,
            "weston" => return WaylandCompositor::Weston,
            _ => {}
        }
    }

    // Check for wlroots-specific environment variables
    if env::var("SWAYSOCK").is_ok()
        || env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok()
        || env::var("RIVER").is_ok()
        || env::var("NIRI_SOCKET").is_ok()
        || env::var("DWL").is_ok() {
        return WaylandCompositor::Wlroots;
    }

    WaylandCompositor::Unknown("undetected".to_string())
}

/// Detect X11 window manager
pub fn detect_x11_wm() -> X11WindowManager {
    // Check environment variables first (fast path)
    if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
        match desktop.as_str() {
            "KDE" => return X11WindowManager::KWin,
            "GNOME" => return X11WindowManager::Mutter,
            "XFCE" => return X11WindowManager::Xfwm,
            "MATE" => return X11WindowManager::Marco,
            "i3" => return X11WindowManager::I3,
            _ => {}
        }
    }

    // TODO: Query X11 directly using EWMH _NET_WM_NAME
    // For now, return generic EWMH as fallback
    X11WindowManager::GenericEWMH("unknown".to_string())
}

/// Get a human-readable name for the detected WM
pub fn get_wm_name() -> String {
    match detect_display_server() {
        DisplayServer::Wayland => {
            match detect_wayland_compositor() {
                WaylandCompositor::Wlroots => "wlroots-based".to_string(),
                WaylandCompositor::KWin => "KWin (Wayland)".to_string(),
                WaylandCompositor::Mutter => "Mutter (Wayland)".to_string(),
                WaylandCompositor::Cosmic => "Cosmic".to_string(),
                WaylandCompositor::Weston => "Weston".to_string(),
                WaylandCompositor::Unknown(s) => format!("Unknown Wayland ({})", s),
            }
        }
        DisplayServer::X11 => {
            match detect_x11_wm() {
                X11WindowManager::KWin => "KWin (X11)".to_string(),
                X11WindowManager::Mutter => "Mutter (X11)".to_string(),
                X11WindowManager::Metacity => "Metacity".to_string(),
                X11WindowManager::Xfwm => "XFWM (XFCE)".to_string(),
                X11WindowManager::Marco => "Marco (MATE)".to_string(),
                X11WindowManager::I3 => "i3".to_string(),
                X11WindowManager::Bspwm => "bspwm".to_string(),
                X11WindowManager::Openbox => "Openbox".to_string(),
                X11WindowManager::Awesome => "Awesome".to_string(),
                X11WindowManager::Dwm => "dwm".to_string(),
                X11WindowManager::Xmonad => "XMonad".to_string(),
                X11WindowManager::GenericEWMH(s) => format!("Unknown X11 ({})", s),
            }
        }
        DisplayServer::Unknown => "Unknown display server".to_string(),
    }
}
