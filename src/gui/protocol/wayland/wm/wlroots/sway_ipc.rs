// Sway IPC Socket Client
// Conecta al socket $SWAYSOCK para obtener geometría en tiempo real

use super::super::WindowGeometry;
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
