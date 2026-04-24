// Esta implementacion existe para poder obtener la geometria de las ventanas de wlroots osea en wm basados en wayland (como sway, hyprland, river, niri, dwl, etc.) y en base a eso posicionar el editor en funcion de la ventana activa a la que apunta osea la terminal que se esta usando para ejecutar el programa

use super::WindowGeometry;

use wayland_client::{Connection, Dispatch, QueueHandle, protocol::wl_registry};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
    zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
};
use std::collections::HashMap;

pub struct WlrootsWindowManager {
    connection: Connection,
}

impl WlrootsWindowManager {
    pub fn new() -> Self {
        let connection = Connection::connect_to_env()
            .expect("Failed to connect to Wayland display");

        Self { connection }
    }
}

impl super::WindowManager for WlrootsWindowManager {
    fn get_window_geometry(&self, target_app_id: &str) -> Option<super::WindowGeometry> {
        let mut state = WmState::new(target_app_id.to_string());
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
        for (handle, info) in &state.toplevels {
            if let Some(ref app_id) = info.app_id {
                if app_id == target_app_id {
                    // Por ahora devolvemos una geometría calculada
                    // En una implementación completa necesitaríamos obtener
                    // la geometría real del output o usar xdg-output
                    return Some(WindowGeometry::new(
                        0, // x - necesitaríamos obtener del output
                        30, // y - aproximado para waybar
                        890, // width - valor por defecto
                        1034, // height - valor por defecto
                    ));
                }
            }
        }

        None
    }
}

struct ToplevelInfo {
    app_id: Option<String>,
    title: Option<String>,
}

struct WmState {
    target_app_id: String,
    toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
    toplevels: HashMap<ZwlrForeignToplevelHandleV1, ToplevelInfo>,
}

impl WmState {
    fn new(target_app_id: String) -> Self {
        Self {
            target_app_id,
            toplevel_manager: None,
            toplevels: HashMap::new(),
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for WmState {
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

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for WmState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: <ZwlrForeignToplevelManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
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

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for WmState {
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
