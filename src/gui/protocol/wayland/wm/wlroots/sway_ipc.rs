// Sway IPC Socket Client
// Conecta al socket $SWAYSOCK para obtener geometría en tiempo real

use super::super::{WindowGeometry, WmEvent};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
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
        sender: mpsc::Sender<WindowGeometry>,
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
    ///
    /// target_toplevel_id: Optional foreign_toplevel_identifier to identify the exact window instance
    /// capture_signal_rx: Channel receiver that triggers monitoring start
    pub fn subscribe_window_events(
        &self,
        target_app_id: String,
        target_toplevel_id: Option<String>,
        sender: mpsc::Sender<WmEvent>,
        capture_signal_rx: mpsc::Receiver<()>,
    ) -> Result<(), String> {
        thread::spawn(move || {
            // Mutable target_toplevel_id - can be captured from first matching event if not provided
            let mut target_toplevel_id_opt = target_toplevel_id;
            println!("[SwayIPC] Starting swaymsg subscribe for app_id: {:?}, toplevel_id: {:?}", target_app_id, target_toplevel_id_opt);

            // Spawn swaymsg process to subscribe to window and workspace events
            let mut child = match Command::new("swaymsg")
                .args(["-t", "subscribe", "-m", "[\"window\", \"workspace\"]"])
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
            println!("[SwayIPC] Waiting for capture signal before attempting toplevel_id capture...");

            // Wait for signal to start monitoring
            match capture_signal_rx.recv() {
                Ok(_) => println!("[SwayIPC] Capture signal received, starting monitoring..."),
                Err(e) => {
                    eprintln!("[SwayIPC] Capture signal channel closed: {}", e);
                    return;
                }
            }

            println!("[SwayIPC] Monitoring app_id: {}", target_app_id);

            // Read events line by line (each line is a JSON event)
            for line in reader.lines() {
                match line {
                    Ok(json_str) => {
                        if json_str.trim().is_empty() {
                            continue;
                        }

                        // Parse and process the event
                        if let Ok(event) = serde_json::from_str::<WindowEvent>(&json_str) {
                            // If we don't have toplevel_id yet, try to capture it from matching app_id event
                            if target_toplevel_id_opt.is_none() {
                                if event.container.app_id.as_deref() == Some(&target_app_id) {
                                    if let Some(ref id) = event.container.foreign_toplevel_identifier {
                                        println!("[SwayIPC] Captured toplevel_id: {} from first matching event", id);
                                        target_toplevel_id_opt = Some(id.clone());
                                    }
                                }
                            }

                            // Process all events - fainting_trigger will filter by toplevel_id
                            Self::fainting_trigger(event, &target_app_id, target_toplevel_id_opt.as_deref(), &sender);
                        } else {
                            // Not a window event - likely workspace event
                            // Verify visibility of our window
                            println!("[SwayIPC] Workspace event detected - checking visibility");
                            if let Some(ref toplevel_id) = target_toplevel_id_opt {
                                match Self::new() {
                                    Ok(client) => {
                                        match client.get_window_visibility_by_toplevel_id(toplevel_id) {
                                            Some(false) => {
                                                // Window not visible - hide overlay
                                                println!("[SwayIPC] Window not visible after workspace change - hiding overlay");
                                                let _ = sender.send(WmEvent::WindowUnfocused {
                                                    app_id: target_app_id.to_string()
                                                });
                                            }
                                            Some(true) => {
                                                println!("[SwayIPC] Window still visible after workspace change");
                                            }
                                            None => {
                                                println!("[SwayIPC] ERROR: Could not query visibility after workspace change");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("[SwayIPC] ERROR: Failed to create SwayIpcClient: {}", e);
                                    }
                                }
                            }
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

    fn process_window_event(event: WindowEvent, target_app_id: &str, sender: &mpsc::Sender<WmEvent>) {
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
        target_toplevel_id: Option<&str>,
        sender: &mpsc::Sender<WmEvent>,
    ) {
        let event_toplevel_id = event.container.foreign_toplevel_identifier.as_deref();

        // Calculate geometry for our window (in case we need it)
        let global_rect = &event.container.rect;
        let window_rect = &event.container.window_rect;
        let geometry = WindowGeometry {
            x: global_rect.x + window_rect.x,
            y: global_rect.y + window_rect.y,
            width: window_rect.width,
            height: window_rect.height,
        };

        // Check if this event is from our target window (using toplevel_id if available)
        let is_our_window = target_toplevel_id.is_some()
            && event_toplevel_id == target_toplevel_id;

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
                "move" | "resize" => {
                    if event.container.focused {
                        Self::geometry_trigger(event, target_app_id, sender);
                    }
                }
                "fullscreen" => {
                    if event.container.focused {
                        Self::fullscreen_trigger(event, target_app_id, sender);
                    }
                }
                _ => {
                    // Other changes - ignore
                }
            }
        } else if event.change.as_str() == "focus" && event.container.focused {
            // Another window gained focus - check if our window is still visible
            println!("[SwayIPC] === TRIGGER ACTIVATED ===");
            println!("[SwayIPC] Another window focused: app_id={:?}, toplevel_id={:?}",
                event.container.app_id, event.container.foreign_toplevel_identifier);

            // Query sway for current tree to check our window's visibility by toplevel_id
            if let Some(toplevel_id) = target_toplevel_id {
                println!("[SwayIPC] Querying visibility for toplevel_id: {}", toplevel_id);

                match Self::new() {
                    Ok(client) => {
                        println!("[SwayIPC] SwayIpcClient created successfully");

                        match client.get_window_visibility_by_toplevel_id(toplevel_id) {
                            Some(visible) => {
                                println!("[SwayIPC] Query result: visible={}", visible);

                                if visible {
                                    println!("[SwayIPC] Our window is visible - keeping overlay visible");
                                    let _ = sender.send(WmEvent::WindowFocused {
                                        app_id: target_app_id.to_string()
                                    });
                                } else {
                                    println!("[SwayIPC] Our window is not visible - hiding overlay");
                                    let _ = sender.send(WmEvent::WindowUnfocused {
                                        app_id: target_app_id.to_string()
                                    });
                                }
                            }
                            None => {
                                println!("[SwayIPC] ERROR: Window with toplevel_id {} not found in tree!", toplevel_id);
                            }
                        }
                    }
                    Err(e) => {
                        println!("[SwayIPC] ERROR: Failed to create SwayIpcClient: {}", e);
                    }
                }
            } else {
                println!("[SwayIPC] WARNING: target_toplevel_id is None, cannot query visibility");
            }

            println!("[SwayIPC] === TRIGGER COMPLETED ===");
        }
    }

    /// Get window visibility by foreign_toplevel_identifier using robust grep filtering
    fn get_window_visibility_by_toplevel_id(&self, toplevel_id: &str) -> Option<bool> {
        println!("[SwayIPC] get_window_visibility_by_toplevel_id called for: {}", toplevel_id);

        // Use shell command with chained grep for robust filtering
        // First grep captures 20 lines after toplevel_id match, second grep extracts visible field
        let toplevel_str = format!("\"foreign_toplevel_identifier\": \"{}\"", toplevel_id);
        let cmd = format!("swaymsg -t get_tree | grep -A20 '{}' | grep \"visible\"", toplevel_str);

        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .map_err(|e| {
                println!("[SwayIPC] ERROR: Failed to run grep command: {}", e);
                e
            }).ok()?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        println!("[SwayIPC] grep output:\n{}", output_str);

        // Parse the visible line - should be something like: "visible": true,
        for line in output_str.lines() {
            if line.contains("\"visible\"") {
                let visible = line.contains("true");
                println!("[SwayIPC] Found visible={}", visible);
                return Some(visible);
            }
        }

        println!("[SwayIPC] ERROR: visible field not found in grep output");
        None
    }

    /// Trigger for move and resize events - updates overlay position and size
    fn geometry_trigger(
        event: WindowEvent,
        target_app_id: &str,
        sender: &mpsc::Sender<WmEvent>,
    ) {
        // Calculate geometry using rect + window_rect
        let global_rect = &event.container.rect;
        let window_rect = &event.container.window_rect;

        let geometry = WindowGeometry {
            x: global_rect.x + window_rect.x,
            y: global_rect.y + window_rect.y,
            width: window_rect.width,
            height: window_rect.height,
        };

        println!("[SwayIPC] Geometry trigger - move/resize: x={}, y={}, w={}, h={}",
            geometry.x, geometry.y, geometry.width, geometry.height);

        let _ = sender.send(WmEvent::GeometryChanged {
            app_id: target_app_id.to_string(),
            geometry,
        });
    }

    /// Trigger for fullscreen events - updates overlay position, size, and layer shell
    fn fullscreen_trigger(
        event: WindowEvent,
        target_app_id: &str,
        sender: &mpsc::Sender<WmEvent>,
    ) {
        // Calculate fullscreen geometry
        let global_rect = &event.container.rect;
        let window_rect = &event.container.window_rect;

        let geometry = WindowGeometry {
            x: global_rect.x + window_rect.x,
            y: global_rect.y + window_rect.y,
            width: window_rect.width,
            height: window_rect.height,
        };

        // Check if window is in fullscreen mode (fullscreen_mode > 0 means fullscreen)
        let is_fullscreen = event.container.fullscreen_mode.unwrap_or(0) > 0;

        println!("[SwayIPC] Fullscreen trigger: is_fullscreen={}, geometry={:?}",
            is_fullscreen, geometry);

        let _ = sender.send(WmEvent::FullscreenChanged {
            app_id: target_app_id.to_string(),
            geometry,
            is_fullscreen,
        });
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

    /// Get the full window tree using swaymsg CLI
    fn get_tree(&self) -> Result<Node, String> {
        let output = Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
            .map_err(|e| format!("Failed to run swaymsg get_tree: {}", e))?;

        if !output.status.success() {
            return Err(format!("swaymsg get_tree failed with exit code: {:?}", output.status.code()));
        }

        let tree_json = String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF8 in swaymsg output: {}", e))?;

        serde_json::from_str(&tree_json)
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
    pid: Option<i64>,
    foreign_toplevel_identifier: Option<String>,
    name: Option<String>,
    focused: bool,
    visible: bool,
    fullscreen_mode: Option<i64>,
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
    pid: Option<i64>,
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
