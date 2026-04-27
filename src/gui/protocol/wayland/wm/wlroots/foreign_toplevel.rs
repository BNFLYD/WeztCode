// Foreign Toplevel Protocol - Wayland
// Detecta ventanas y su estado de foco

use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_registry};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
    zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    zwlr_foreign_toplevel_handle_v1::Event,
};
use std::collections::HashMap;
use std::sync::mpsc;
use super::super::WmEvent;

pub struct ForeignToplevelDetector {
    connection: Connection,
}

impl ForeignToplevelDetector {
    pub fn new() -> Result<Self, String> {
        let connection = Connection::connect_to_env()
            .map_err(|e| format!("Failed to connect to Wayland display: {}", e))?;

        Ok(Self { connection })
    }

    pub fn detect_window(&self, target_app_id: &str) -> Option<WindowInfo> {
        let mut state = ToplevelState::new(target_app_id.to_string());
        let mut event_queue = self.connection.new_event_queue();
        let qh = event_queue.handle();

        let display = self.connection.display();
        display.get_registry(&qh, ());

        // Roundtrip para obtener el registry
        event_queue.roundtrip(&mut state).ok()?;

        // Si encontramos el toplevel manager, obtener toplevels
        if let Some(ref manager) = state.toplevel_manager {
            manager.stop();
        }

        // Roundtrip para procesar eventos
        event_queue.roundtrip(&mut state).ok()?;

        // Buscar el toplevel con el app_id objetivo
        for (_handle, info) in &state.toplevels {
            if let Some(ref app_id) = info.app_id {
                if app_id == target_app_id {
                    return Some(WindowInfo {
                        app_id: app_id.clone(),
                        title: info.title.clone(),
                        is_focused: false, // TODO: detectar foco
                    });
                }
            }
        }

        None
    }
}

pub struct WindowInfo {
    pub app_id: String,
    pub title: Option<String>,
    pub is_focused: bool,
}

struct ToplevelInfo {
    app_id: Option<String>,
    title: Option<String>,
    is_focused: bool,
}

struct ToplevelState {
    target_app_id: String,
    toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
    toplevels: HashMap<ZwlrForeignToplevelHandleV1, ToplevelInfo>,
}

impl ToplevelState {
    fn new(target_app_id: String) -> Self {
        Self {
            target_app_id,
            toplevel_manager: None,
            toplevels: HashMap::new(),
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == "zwlr_foreign_toplevel_manager_v1" {
                let manager = registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(
                    name,
                    version.min(3),
                    qh,
                    (),
                );
                state.toplevel_manager = Some(manager);
            }
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: <ZwlrForeignToplevelManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::Event;
        match event {
            Event::Toplevel { toplevel } => {
                state.toplevels.insert(toplevel, ToplevelInfo {
                    app_id: None,
                    title: None,
                    is_focused: false,
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::Event;
        if let Some(info) = state.toplevels.get_mut(handle) {
            match event {
                Event::AppId { app_id } => {
                    info.app_id = Some(app_id);
                }
                Event::Title { title } => {
                    info.title = Some(title);
                }
                Event::State { state: state_data } => {
                    // Parse state: activated = 1 (focused)
                    info.is_focused = parse_state(&state_data);
                }
                _ => {}
            }
        }
    }
}

fn parse_state(state_data: &[u8]) -> bool {
    // State array: [u32 type, ...]
    // Type 0 = activated (focused)
    if state_data.len() >= 4 {
        let state_type = u32::from_ne_bytes([state_data[0], state_data[1], state_data[2], state_data[3]]);
        return state_type == 1; // activated
    }
    false
}

/// Inicia monitoreo continuo de cambios de foco para el app_id especificado
/// Envía eventos WmEvent a través del channel proporcionado
pub fn start_focus_monitor(target_app_id: String, event_sender: mpsc::Sender<WmEvent>) -> Result<(), String> {
    std::thread::spawn(move || {
        let connection = match Connection::connect_to_env() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to connect to Wayland: {}", e);
                return;
            }
        };

        let mut state = FocusMonitorState::new(target_app_id, event_sender);
        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();

        let display = connection.display();
        display.get_registry(&qh, ());

        loop {
            if let Err(e) = event_queue.roundtrip(&mut state) {
                eprintln!("Wayland event loop error: {}", e);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    Ok(())
}

struct FocusMonitorState {
    target_app_id: String,
    event_sender: mpsc::Sender<WmEvent>,
    toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
    toplevels: HashMap<ZwlrForeignToplevelHandleV1, ToplevelInfo>,
    target_was_focused: bool,  // Track previous state to detect changes
}

impl FocusMonitorState {
    fn new(target_app_id: String, event_sender: mpsc::Sender<WmEvent>) -> Self {
        Self {
            target_app_id,
            event_sender,
            toplevel_manager: None,
            toplevels: HashMap::new(),
            target_was_focused: false,
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for FocusMonitorState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == "zwlr_foreign_toplevel_manager_v1" {
                let manager = registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(
                    name,
                    version.min(3),
                    qh,
                    (),
                );
                state.toplevel_manager = Some(manager);
            }
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for FocusMonitorState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: <ZwlrForeignToplevelManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::Event;
        match event {
            Event::Toplevel { toplevel } => {
                state.toplevels.insert(toplevel, ToplevelInfo {
                    app_id: None,
                    title: None,
                    is_focused: false,
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for FocusMonitorState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::Event;
        if let Some(info) = state.toplevels.get_mut(handle) {
            match event {
                Event::AppId { app_id } => {
                    info.app_id = Some(app_id);
                }
                Event::Title { title } => {
                    info.title = Some(title);
                }
                Event::State { state: state_data } => {
                    let was_focused_before = info.is_focused;
                    info.is_focused = parse_state(&state_data);

                    // Check if this is our target window and focus state changed
                    if info.app_id.as_ref() == Some(&state.target_app_id) {
                        if info.is_focused && !was_focused_before && !state.target_was_focused {
                            // Window just gained focus
                            let _ = state.event_sender.send(
                                WmEvent::WindowFocused {
                                    app_id: state.target_app_id.clone()
                                }
                            );
                            state.target_was_focused = true;
                        } else if !info.is_focused && (was_focused_before || state.target_was_focused) {
                            // Window just lost focus
                            let _ = state.event_sender.send(
                                WmEvent::WindowUnfocused {
                                    app_id: state.target_app_id.clone()
                                }
                            );
                            state.target_was_focused = false;
                        }
                    }
                }
                Event::Closed => {
                    if info.app_id.as_ref() == Some(&state.target_app_id) {
                        if state.target_was_focused {
                            // Window was focused but now closed
                            let _ = state.event_sender.send(
                                WmEvent::WindowUnfocused {
                                    app_id: state.target_app_id.clone()
                                }
                            );
                            state.target_was_focused = false;
                        }
                    }
                    state.toplevels.remove(handle);
                }
                _ => {}
            }
        }
    }
}
