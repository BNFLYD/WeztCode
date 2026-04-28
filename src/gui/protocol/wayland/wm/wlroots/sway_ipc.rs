// Sway IPC Socket Client
// Conecta al socket $SWAYSOCK para obtener geometría en tiempo real

use super::super::{WindowGeometry, WmEvent};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
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
    /// Subscribe to window events and send WmEvent through channel
    pub fn subscribe_window_events(
        &self,
        target_app_id: String,
        sender: Sender<WmEvent>,
    ) -> Result<(), String> {
        let socket_path = self.socket_path.clone();

        thread::spawn(move || {
            let mut stream = match UnixStream::connect(&socket_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[SwayIPC] Failed to connect: {}", e);
                    return;
                }
            };

            // Subscribe to window events (type 2 = SUBSCRIBE)
            let payload = r#"["window"]"#;
            if let Err(e) = Self::send_message(&mut stream, 2u32, payload) {
                eprintln!("[SwayIPC] Failed to subscribe: {}", e);
                return;
            }

            println!("[SwayIPC] Subscribed to window events");

            // Read events loop - Sway IPC uses binary message format
            let mut stream = stream;
            let mut header_buf = [0u8; 14]; // 6 magic + 4 type + 4 length

            loop {
                println!("[SwayIPC] Waiting for next event...");
                // Read header (14 bytes)
                match stream.read_exact(&mut header_buf) {
                    Ok(()) => {
                        // Parse payload length (bytes 10-13, little endian)
                        let payload_len = u32::from_ne_bytes([header_buf[10], header_buf[11], header_buf[12], header_buf[13]]) as usize;

                        if payload_len > 0 && payload_len < 10_000_000 { // Sanity check
                            let mut payload_buf = vec![0u8; payload_len];

                            match stream.read_exact(&mut payload_buf) {
                                Ok(()) => {
                                    if let Ok(payload_str) = String::from_utf8(payload_buf) {
                                        println!("[SwayIPC] Raw event: {}", payload_str.chars().take(200).collect::<String>());

                                        // Try to parse as window event first
                                        if let Ok(event) = serde_json::from_str::<WindowEvent>(&payload_str) {
                                            Self::process_window_event(event, &target_app_id, &sender);
                                        } else {
                                            // Might be a subscribe response or other message - ignore
                                            println!("[SwayIPC] Not a window event, skipping");
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[SwayIPC] Failed to read payload: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[SwayIPC] Connection error: {}", e);
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    fn process_window_event(event: WindowEvent, target_app_id: &str, sender: &Sender<WmEvent>) {
        let app_id = event.container.app_id.as_deref().unwrap_or("");

        // Only process events for our target window
        if app_id != target_app_id {
            return;
        }

        let rect = event.container.rect;
        let geometry = WindowGeometry {
            x: rect.x,
            y: rect.y,
            width: rect.width as i32,
            height: rect.height as i32,
        };

        match event.change.as_str() {
            "focus" => {
                println!("[SwayIPC] Target window FOCUSED");
                let _ = sender.send(WmEvent::WindowFocused {
                    app_id: target_app_id.to_string(),
                });
            }
            "unfocus" => {
                println!("[SwayIPC] Target window UNFOCUSED");
                let _ = sender.send(WmEvent::WindowUnfocused {
                    app_id: target_app_id.to_string(),
                });
            }
            "move" | "resize" | "floating" | "tile" => {
                println!("[SwayIPC] Target window geometry changed: {:?}", geometry);
                let _ = sender.send(WmEvent::GeometryChanged {
                    app_id: target_app_id.to_string(),
                    geometry,
                });
            }
            _ => {}
        }
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
    name: Option<String>,
    rect: Rect,
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
    rect: Option<Rect>,
    nodes: Vec<Node>,
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
