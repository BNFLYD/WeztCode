// Sway IPC Socket Client
// Conecta al socket $SWAYSOCK para obtener geometría en tiempo real

use super::super::{WindowGeometry, WmEvent};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;
use serde::Deserialize;

pub struct SwayIpcClient {
    socket_path: String,
}

impl SwayIpcClient {
    pub fn new() -> Result<Self, String> {
        let socket_path = std::env::var("SWAYSOCK")
            .map_err(|_| "SWAYSOCK environment variable not set".to_string())?;

        Ok(Self { socket_path })
    }

    /// Obtiene la geometría de una ventana por su app_id
    pub fn get_window_geometry(&self, target_app_id: &str) -> Option<WindowGeometry> {
        let tree = self.get_tree().ok()?;

        // Buscar la ventana en el árbol
        for node in tree.nodes.iter().flat_map(|n| flatten_nodes(n)) {
            if let Some(ref app_id) = node.app_id {
                if app_id == target_app_id {
                    let rect = node.rect?;
                    return Some(WindowGeometry::new(
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                    ));
                }
            }
        }

        None
    }

    /// Inicia un listener en un thread separado para monitorear cambios
    pub fn subscribe_geometry_changes(
        &self,
        target_app_id: String,
        sender: Sender<WindowGeometry>,
    ) -> Result<thread::JoinHandle<()>, String> {
        let _socket_path = self.socket_path.clone();

        let handle = thread::spawn(move || {
            // TODO: Implementar suscripción a eventos de ventana
            // Por ahora hacemos polling cada 100ms
            loop {
                if let Ok(client) = SwayIpcClient::new() {
                    if let Some(geo) = client.get_window_geometry(&target_app_id) {
                        let _ = sender.send(geo);
                    }
                }
                thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        Ok(handle)
    }

    /// Inicia un listener en un thread separado para monitorear cambios
    /// Subscribe to window events using swaymsg CLI and send WmEvent through channel
    pub fn subscribe_window_events(
        &self,
        target_app_id: String,
        sender: Sender<WmEvent>,
    ) -> Result<(), String> {
        thread::spawn(move || {
            println!("[SwayIPC] Starting swaymsg subscribe for app_id: {}", target_app_id);

            // Spawn swaymsg process to subscribe to window events
            let mut child = match Command::new("swaymsg")
                .args(["-t", "subscribe", "-m", "[\"window\"]"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[SwayIPC] Failed to spawn swaymsg: {}", e);
                    return;
                }
            };

            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    eprintln!("[SwayIPC] Failed to get stdout from swaymsg");
                    return;
                }
            };

            let reader = BufReader::new(stdout);
            println!("[SwayIPC] Listening for window events...");

            // Track our target window's toplevel identifier
            let mut target_toplevel_id: Option<String> = None;

            // Read events line by line (each line is a JSON event)
            for line in reader.lines() {
                match line {
                    Ok(json_str) => {
                        if json_str.trim().is_empty() {
                            continue;
                        }

                        // Parse and process the event
                        if let Ok(event) = serde_json::from_str::<WindowEvent>(&json_str) {
                            // Capture toplevel_id when we first see our target app
                            if target_toplevel_id.is_none() {
                                if let Some(ref app_id) = event.container.app_id {
                                    if app_id == &target_app_id {
                                        if let Some(ref toplevel_id) = event.container.foreign_toplevel_identifier {
                                            target_toplevel_id = Some(toplevel_id.clone());
                                            println!("[SwayIPC] Captured target toplevel_id: {}", toplevel_id);
                                        }
                                    }
                                }
                            }

                            // Process all events through the event-driven logic
                            Self::fainting_trigger(event, &target_app_id, target_toplevel_id.as_ref(), &sender);
                        }
                    }
                    Err(e) => {
                        eprintln!("[SwayIPC] Error reading line: {}", e);
                        break;
                    }
                }
            }

            eprintln!("[SwayIPC] Event loop ended, swaymsg exited");
        });

        Ok(())
    }

    fn process_window_event(event: WindowEvent, target_app_id: &str, sender: &Sender<WmEvent>) {
        let app_id = event.container.app_id.as_deref().unwrap_or("");

        // Only process events for our target window
        if app_id != target_app_id {
            return;
        }

        // Calculate overlay geometry: rect (global position) + window_rect (internal offset and size)
        let global_rect = &event.container.rect;
        let window_rect = &event.container.window_rect;

        let geometry = WindowGeometry {
            x: global_rect.x + window_rect.x,
            y: global_rect.y + window_rect.y,
            width: window_rect.width,
            height: window_rect.height,
        };

        // Get app_id for event
        let event_app_id = event.container.app_id.clone().unwrap_or_default();

        // Determine event type based on change field and focused status
        match event.change.as_str() {
            "focus" => {
                if event.container.focused {
                    println!("[SwayIPC] Target window FOCUSED at {:?}", geometry);
                    let _ = sender.send(WmEvent::WindowFocused { app_id: event_app_id });
                    let _ = sender.send(WmEvent::GeometryChanged { app_id: target_app_id.to_string(), geometry });
                }
            }
            "unfocus" => {
                println!("[SwayIPC] Target window UNFOCUSED");
                let _ = sender.send(WmEvent::WindowUnfocused { app_id: event_app_id });
            }
            "move" | "resize" | "fullscreen" => {
                if event.container.focused {
                    println!("[SwayIPC] Target window GEOMETRY CHANGED: {:?}", geometry);
                    let _ = sender.send(WmEvent::GeometryChanged { app_id: target_app_id.to_string(), geometry });
                }
            }
            _ => {
                // Ignore other changes (title, urgent, etc.)
            }
        }
    }

    /// Process window events using foreign_toplevel_identifier for precise tracking
    fn fainting_trigger(
        event: WindowEvent,
        target_app_id: &str,
        target_toplevel_id: Option<&String>,
        sender: &Sender<WmEvent>,
    ) {
        let event_toplevel_id = event.container.foreign_toplevel_identifier.as_deref();
        let app_id = event.container.app_id.as_deref().unwrap_or("");

        // Calculate geometry for our window (in case we need it)
        let global_rect = &event.container.rect;
        let window_rect = &event.container.window_rect;
        let geometry = WindowGeometry {
            x: global_rect.x + window_rect.x,
            y: global_rect.y + window_rect.y,
            width: window_rect.width,
            height: window_rect.height,
        };

        // Check if this event is from our target window
        let is_our_window = target_toplevel_id.is_some()
            && event_toplevel_id == target_toplevel_id.as_deref();

        if is_our_window {
            // Our window event - process normally
            match event.change.as_str() {
                "focus" => {
                    if event.container.focused {
                        println!("[SwayIPC] Target window FOCUSED (toplevel match)");
                        let _ = sender.send(WmEvent::WindowFocused {
                            app_id: target_app_id.to_string()
                        });
                        let _ = sender.send(WmEvent::GeometryChanged {
                            app_id: target_app_id.to_string(),
                            geometry
                        });
                    }
                }
                "move" | "resize" | "fullscreen" => {
                    if event.container.focused {
                        println!("[SwayIPC] Target window GEOMETRY CHANGED (toplevel match)");
                        let _ = sender.send(WmEvent::GeometryChanged {
                            app_id: target_app_id.to_string(),
                            geometry
                        });
                    }
                }
                _ => {
                    // Other changes - ignore
                }
            }
        } else if event.change.as_str() == "focus" && event.container.focused {
            // Another window gained focus - check if our window is still visible
            println!("[SwayIPC] Another window focused, checking our visibility...");

            // Query sway for current tree to check our window's visibility
            if let Some(toplevel_id) = target_toplevel_id {
                if let Ok(client) = Self::new() {
                    if let Some((visible, is_focused)) = client.get_window_visibility_by_toplevel(toplevel_id) {
                        if visible {
                            if is_focused {
                                println!("[SwayIPC] Our window is visible and focused - keeping overlay visible");
                                let _ = sender.send(WmEvent::WindowFocused {
                                    app_id: target_app_id.to_string()
                                });
                            } else {
                                println!("[SwayIPC] Our window is visible but not focused - keeping overlay visible (side-by-side)");
                                // In side-by-side mode, keep overlay visible even without focus
                                let _ = sender.send(WmEvent::WindowFocused {
                                    app_id: target_app_id.to_string()
                                });
                            }
                        } else {
                            println!("[SwayIPC] Our window is not visible - hiding overlay");
                            let _ = sender.send(WmEvent::WindowUnfocused {
                                app_id: target_app_id.to_string()
                            });
                        }
                    }
                }
            }
        }
    }

    /// Get window visibility and focus status by toplevel identifier
    fn get_window_visibility_by_toplevel(&self, target_toplevel_id: &str) -> Option<(bool, bool)> {
        let tree = self.get_tree().ok()?;

        // Search through all nodes for matching toplevel_id
        for node in tree.nodes.iter().flat_map(|n| flatten_nodes(n)) {
            if let Some(ref toplevel_id) = node.foreign_toplevel_identifier {
                if toplevel_id == target_toplevel_id {
                    return Some((node.visible, node.focused));
                }
            }
        }

        None
    }

    fn send_message(stream: &mut UnixStream, msg_type: u32, payload: &str) -> Result<(), String> {
        let magic = b"i3-ipc";
        let payload_bytes = payload.as_bytes();

        stream.write_all(magic).map_err(|e| e.to_string())?;
        stream.write_all(&msg_type.to_ne_bytes()).map_err(|e| e.to_string())?;
        stream.write_all(&(payload_bytes.len() as u32).to_ne_bytes()).map_err(|e| e.to_string())?;
        stream.write_all(payload_bytes).map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Ejecuta un comando IPC y devuelve la respuesta
    fn run_command(&self, command: &str) -> Result<IpcResponse, String> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| format!("Failed to connect to sway IPC socket: {}", e))?;

        // Formato del mensaje: [magic: 4 bytes] [type: 4 bytes] [len: 4 bytes] [payload: len bytes]
        let magic = b"i3-ipc";
        let msg_type = 0u32; // RUN_COMMAND
        let payload = command.as_bytes();

        stream.write_all(magic).map_err(|e| e.to_string())?;
        stream.write_all(&msg_type.to_ne_bytes()).map_err(|e| e.to_string())?;
        stream.write_all(&(payload.len() as u32).to_ne_bytes()).map_err(|e| e.to_string())?;
        stream.write_all(payload).map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;

        // Leer respuesta
        let mut header = [0u8; 14];
        stream.read_exact(&mut header).map_err(|e| e.to_string())?;

        // TODO: Parsear la respuesta correctamente
        let payload_len = u32::from_ne_bytes([header[10], header[11], header[12], header[13]]);
        let mut payload = vec![0u8; payload_len as usize];
        stream.read_exact(&mut payload).map_err(|e| e.to_string())?;

        let response_str = String::from_utf8(payload).map_err(|e| e.to_string())?;

        Ok(IpcResponse {
            success: true,
            payload: response_str,
        })
    }

    fn get_tree(&self) -> Result<Node, String> {
        let response = self.run_command("get_tree")?;
        serde_json::from_str(&response.payload)
            .map_err(|e| format!("Failed to parse tree JSON: {}", e))
    }
}

/// Window event from Sway IPC subscription
#[derive(Debug, Deserialize)]
struct WindowEvent {
    change: String,
    container: WindowEventContainer,
}

/// Container data in window event
#[derive(Debug, Deserialize)]
struct WindowEventContainer {
    id: i64,
    app_id: Option<String>,
    foreign_toplevel_identifier: Option<String>,
    name: Option<String>,
    focused: bool,
    visible: bool,
    rect: Rect,
    window_rect: Rect,
}

#[derive(Debug, Deserialize)]
struct IpcResponse {
    success: bool,
    payload: String,
}

#[derive(Debug, Deserialize)]
struct Node {
    id: i64,
    name: Option<String>,
    node_type: Option<String>,
    app_id: Option<String>,
    foreign_toplevel_identifier: Option<String>,
    rect: Option<Rect>,
    nodes: Vec<Node>,
    focused: bool,
    visible: bool,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn flatten_nodes(node: &Node) -> Vec<&Node> {
    let mut result = vec![node];
    for child in &node.nodes {
        result.extend(flatten_nodes(child));
    }
    result
}
