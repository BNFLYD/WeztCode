// Foreign Toplevel Protocol - Wayland
// Detecta ventanas y su estado de foco

use super::super::WindowGeometry;
use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_registry};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
    zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
};
use std::collections::HashMap;

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
                _ => {}
            }
        }
    }
}
