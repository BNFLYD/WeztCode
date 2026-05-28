mod config;
mod gui;
mod terminal;
mod chat;

use gui::{GuiPlatform, Gtk4Platform};
use terminal::{TerminalProtocol, WeztermProtocol};
use std::io::Read;
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::Duration;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

use config::keys::KeysStore;

static CHAT_SERVICE: Lazy<Mutex<chat::ChatService>> = Lazy::new(|| {
    let config = chat::ChatConfig::from_props();
    let backend: Box<dyn chat::AgentBackend> = Box::new(chat::PiAgentBackend::new(config));
    Mutex::new(chat::ChatService::new(backend))
});

fn setup_padding_hook() -> Result<(), String> {
    let user_config = std::env::var("WEZTERM_CONFIG_FILE").ok()
        .or_else(|| {
            let home = if cfg!(target_os = "windows") {
                std::env::var("USERPROFILE").ok()
            } else {
                std::env::var("HOME").ok()
            }?;
            Some(if cfg!(target_os = "windows") {
                format!("{}\\.config\\wezterm\\wezterm.lua", home)
            } else {
                format!("{}/.config/wezterm/wezterm.lua", home)
            })
        })
        .unwrap_or_default();

    std::env::set_var("WEZTCODE_USER_CONFIG", &user_config);

    let props_path = std::env::current_dir()
        .unwrap_or_default()
        .join("user_props.lua");
    std::env::set_var("WEZTCODE_PROPS_FILE", props_path.to_str().unwrap_or(""));

    let temp_dir = if cfg!(target_os = "windows") {
        std::env::var("TEMP").unwrap_or_else(|_| "C:\\Temp".to_string())
    } else {
        std::env::var("XDG_RUNTIME_DIR")
            .or_else(|_| std::env::var("TMPDIR"))
            .unwrap_or_else(|_| "/tmp".to_string())
    };

    let weztcode_dir = std::path::PathBuf::from(&temp_dir).join("weztcode");
    std::fs::create_dir_all(&weztcode_dir)
        .map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let lua_path = weztcode_dir.join(config::LUA_FILE_NAME);
    let lua_content = include_str!("terminal/static/wezterm/weztcode.lua");
    std::fs::write(&lua_path, lua_content)
        .map_err(|e| format!("Failed to write Lua file: {}", e))?;

    let pad_path = weztcode_dir.join(config::PAD_FILE_NAME);
    std::fs::write(&pad_path, "0")
        .map_err(|e| format!("Failed to write pad file: {}", e))?;

    let lua_str = lua_path.to_str()
        .ok_or_else(|| "Invalid Lua path".to_string())?;
    let pad_str = pad_path.to_str()
        .ok_or_else(|| "Invalid pad path".to_string())?;

    let active_path = weztcode_dir.join(config::ACTIVE_PANE_FILE_NAME);
    std::fs::write(&active_path, "0")
        .map_err(|e| format!("Failed to write active_pane file: {}", e))?;
    let active_str = active_path.to_str()
        .ok_or_else(|| "Invalid active_pane path".to_string())?;

    std::env::set_var("WEZTCODE_SESSION", "true");
    std::env::set_var("WEZTERM_CONFIG_FILE", lua_str);
    std::env::set_var("WEZTCODE_PAD_FILE", pad_str);
    std::env::set_var("WEZTCODE_ACTIVE_PANE_FILE", active_str);

//     println!("[Main] Padding hook: WEZTERM_CONFIG_FILE={}", lua_str);
//     println!("[Main] Padding hook: WEZTCODE_PAD_FILE={}", pad_str);
//     println!("[Main] Padding hook: WEZTCODE_USER_CONFIG={}", user_config);

    Ok(())
}

fn start_http_server(port: u16) -> thread::JoinHandle<()> {
    let server = tiny_http::Server::http(format!("127.0.0.1:{}", port)).unwrap();

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();

            if url.starts_with("/api/") {
                handle_api(request, &url);
            } else {
                let path = if url == "/" {
                    "frontend/dist/index.html".to_string()
                } else {
                    format!("frontend/dist{}", url)
                };

                let content_type = if path.ends_with(".js") {
                    "application/javascript"
                } else if path.ends_with(".css") {
                    "text/css"
                } else if path.ends_with(".html") {
                    "text/html"
                } else {
                    "application/octet-stream"
                };

                match read_to_string(&path) {
                    Ok(content) => {
                        let response = tiny_http::Response::from_string(content)
                            .with_header(tiny_http::Header {
                                field: "Content-Type".parse().unwrap(),
                                value: content_type.parse().unwrap(),
                            });
                        let _ = request.respond(response);
                    }
                    Err(_) => {
                        let response = tiny_http::Response::from_string("Not found")
                            .with_status_code(404);
                        let _ = request.respond(response);
                    }
                }
            }
        }
    })
}

fn handle_api(request: tiny_http::Request, url: &str) {
    if url == "/api/chat/send" {
        return handle_chat_send(request);
    }

    if url.starts_with("/api/keys/set") {
        return handle_keys_set(request);
    }

    let root = crate::config::props::UserProps::load()
        .get("current_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let response: tiny_http::Response<std::io::Cursor<Vec<u8>>> = if url.starts_with("/api/terminal/list") {
        handle_terminal_list()
    } else if url.starts_with("/api/terminal/spawn") {
        handle_terminal_spawn()
    } else if url.starts_with("/api/terminal/kill") {
        let pane_id = parse_query_param(url, "pane_id").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        handle_terminal_kill(pane_id)
    } else if url.starts_with("/api/terminal/active-pane") {
        handle_active_pane()
    } else if url.starts_with("/api/terminal/ensure-main") {
        handle_terminal_ensure_main()
    } else if url.starts_with("/api/terminal/activate") {
        let pane_id = parse_query_param(url, "pane_id").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        handle_terminal_activate(pane_id)
    } else if url.starts_with("/api/keys/delete") {
        let name = parse_query_param(url, "name").unwrap_or_default();
        handle_keys_delete(&name)
    } else if url.starts_with("/api/keys/list") {
        handle_keys_list()
    } else if url.starts_with("/api/fs/ls") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| "/".to_string());
        handle_ls(&rel_path, &root)
    } else if url.starts_with("/api/fs/read") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_read(&rel_path, &root)
    } else if url.starts_with("/api/editor/open") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_editor_open(&rel_path, &root)
    } else if url.starts_with("/api/fs/create") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_create(&rel_path, &root)
    } else if url.starts_with("/api/fs/delete") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_delete(&rel_path, &root)
    } else if url.starts_with("/api/fs/rename") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        let new_name = parse_query_param(url, "name").unwrap_or_else(|| String::new());
        handle_rename(&rel_path, &new_name, &root)
    } else if url.starts_with("/api/fs/move") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        let dest = parse_query_param(url, "dest").unwrap_or_else(|| String::new());
        handle_move(&rel_path, &dest, &root)
    } else if url.starts_with("/api/fs/image") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_image(rel_path, &root)
    } else {
        json_error("Unknown API endpoint")
    };

    let _ = request.respond(response);
}

fn handle_ls(rel_path: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::list_dir(rel_path, root) {
        Ok(files) => {
            let display_path = if rel_path.is_empty() || rel_path == "/" {
                "/".to_string()
            } else {
                rel_path.trim_start_matches('/').to_string()
            };
            let data = serde_json::json!({
                "ok": true,
                "data": {
                    "path": display_path,
                    "files": files
                }
            });
            json_response(&data)
        }
        Err(e) => json_error(&e)
    }
}

fn handle_read(rel_path: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if rel_path.trim_start_matches('/').eq_ignore_ascii_case("KEYS.env") {
        return json_error("Access denied");
    }

    match crate::config::fs::read_file(rel_path, root) {
        Ok(content) => {
            let data = serde_json::json!({
                "ok": true,
                "data": {
                    "path": rel_path,
                    "content": content
                }
            });
            json_response(&data)
        }
        Err(e) => json_error(&e)
    }
}

fn handle_editor_open(rel_path: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    eprintln!("[editor_open] rel_path={:?}, root={:?}", rel_path, root);

    let full_path = match crate::config::fs::sanitize_path(rel_path, root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[editor_open] sanitize_path error: {}", e);
            return json_error(&e);
        }
    };

    if !full_path.is_file() {
        return json_error("Not a file");
    }

    match std::process::Command::new("nvim")
        .args([
            "--server", "/tmp/weztcode-nvim.sock",
            "--remote", &full_path.to_string_lossy()
        ])
        .output()
    {
        Ok(o) if o.status.success() => {
            json_response(&serde_json::json!({ "ok": true }))
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            json_error(&format!("nvim --remote failed: {}", stderr))
        }
        Err(e) => {
            json_error(&format!("Failed to run nvim: {}", e))
        }
    }
}

fn handle_create(rel_path: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::create_entry(rel_path, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e)
    }
}

fn handle_delete(rel_path: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::delete_entry(rel_path, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e)
    }
}

fn handle_rename(rel_path: &str, new_name: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::rename_entry(rel_path, new_name, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e)
    }
}

fn handle_move(rel_path: &str, dest: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    match crate::config::fs::move_entry(rel_path, dest, root) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e)
    }
}

fn handle_image(rel_path: String, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let full_path = match crate::config::fs::sanitize_path(&rel_path, root) {
        Ok(p) => p,
        Err(e) => return json_response(&serde_json::json!({ "ok": false, "error": e })),
    };

    if !full_path.is_file() {
        return json_response(&serde_json::json!({ "ok": false, "error": "Not a file" }));
    }

    let ext = full_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let content_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    };

    eprintln!("[image] path={:?}, ext={:?}, content_type={}", rel_path, ext, content_type);

    match crate::config::fs::read_image_bytes(&rel_path, root) {
        Ok(bytes) => {
            let len = bytes.len();
            eprintln!("[image] read ok: {} bytes", len);
            let cursor = std::io::Cursor::new(bytes);
            tiny_http::Response::new(
                tiny_http::StatusCode(200),
                vec![
                    tiny_http::Header {
                        field: "Content-Type".parse().unwrap(),
                        value: content_type.parse().unwrap(),
                    },
                ],
                cursor,
                Some(len),
                None,
            )
        }
        Err(e) => {
            eprintln!("[image] read error: {}", e);
            json_response(&serde_json::json!({ "ok": false, "error": e }))
        },
    }
}

fn is_keys_path(path: &Path) -> bool {
    let canonical = path.canonicalize().unwrap_or_default();
    let keys_path = KeysStore::load_path();
    canonical == keys_path.canonicalize().unwrap_or_default()
}

fn handle_keys_set(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);
    let parsed: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => {
            let _ = request.respond(json_error("Invalid JSON body"));
            return;
        }
    };
    let name = parsed.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let value = parsed.get("value").and_then(|v| v.as_str()).unwrap_or("");
    if name.is_empty() {
        let _ = request.respond(json_error("Missing 'name' field"));
        return;
    }
    match KeysStore::set(name, value) {
        Ok(_) => { let _ = request.respond(json_response(&serde_json::json!({ "ok": true }))); }
        Err(e) => { let _ = request.respond(json_error(&e)); }
    }
}

fn handle_keys_delete(name: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if name.is_empty() {
        return json_error("Missing 'name' query param");
    }
    match KeysStore::delete(name) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

fn handle_keys_list() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let store = KeysStore::load();
    let names = store.list_names();
    json_response(&serde_json::json!({ "ok": true, "keys": names }))
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            let raw = parts.next().unwrap_or("");
            return Some(url_decode(raw));
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                bytes.push(byte);
            }
        } else if c == '+' {
            bytes.push(b' ');
        } else {
            let mut buf = [0u8; 4];
            let encoded = c.encode_utf8(&mut buf);
            bytes.extend_from_slice(encoded.as_bytes());
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn json_response(data: &serde_json::Value) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let body = serde_json::to_string(data).unwrap_or_default();
    tiny_http::Response::from_string(body)
        .with_header(
            tiny_http::Header {
                field: "Content-Type".parse().unwrap(),
                value: "application/json".parse().unwrap(),
            }
        )
}

fn json_error(msg: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let data = serde_json::json!({ "ok": false, "error": msg });
    json_response(&data)
}

fn handle_terminal_list() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let term = WeztermProtocol::new();
    match term.list_panes() {
        Ok(raw) => {
            eprintln!("[terminal_list] raw output: {:?}", raw);
            let lines: Vec<String> = raw.lines()
                .skip(1)
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect();
            let data = serde_json::json!({ "ok": true, "data": lines });
            json_response(&data)
        }
        Err(e) => json_error(&e),
    }
}

fn handle_terminal_spawn() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let cwd = crate::config::props::UserProps::load()
        .get("current_dir")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let term = WeztermProtocol::new();
    match term.spawn_tab(cwd.as_deref()) {
        Ok(pane_id) => {
            let data = serde_json::json!({ "ok": true, "data": { "pane_id": pane_id } });
            json_response(&data)
        }
        Err(e) => json_error(&e),
    }
}

fn handle_terminal_kill(pane_id: u32) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if pane_id == 0 {
        return json_error("Invalid pane_id");
    }
    let term = WeztermProtocol::new();
    match term.kill_pane(pane_id) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

fn handle_terminal_activate(pane_id: u32) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    if pane_id == 0 {
        return json_error("Invalid pane_id");
    }
    let term = WeztermProtocol::new();
    match term.activate_pane(pane_id) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

fn read_request_body(request: &mut tiny_http::Request) -> String {
    let mut body = String::new();
    let _ = request.as_reader().read_to_string(&mut body);
    body
}

fn handle_chat_send(mut request: tiny_http::Request) {
    let body = read_request_body(&mut request);

    let message = match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(val) => val.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        Err(_) => {
            let err = json_error("Invalid JSON body");
            let _ = request.respond(err);
            return;
        }
    };

    if message.is_empty() {
        let err = json_error("Message is empty");
        let _ = request.respond(err);
        return;
    }

    let reader = {
        let mut service = match CHAT_SERVICE.lock() {
            Ok(s) => s,
            Err(_) => {
                let err = json_error("Chat service lock failed");
                let _ = request.respond(err);
                return;
            }
        };

        match service.send_message_stream(&message) {
            Ok(r) => r,
            Err(e) => {
                let safe = config::keys::redact_keys(&format!("Chat error: {}", e));
                let err = json_error(&safe);
                let _ = request.respond(err);
                return;
            }
        }
    };

    let response = tiny_http::Response::new(
        tiny_http::StatusCode(200),
        vec![
            tiny_http::Header {
                field: "Content-Type".parse().unwrap(),
                value: "text/event-stream".parse().unwrap(),
            },
            tiny_http::Header {
                field: "Cache-Control".parse().unwrap(),
                value: "no-cache".parse().unwrap(),
            },
            tiny_http::Header {
                field: "Access-Control-Allow-Origin".parse().unwrap(),
                value: "*".parse().unwrap(),
            },
        ],
        reader,
        None,
        None,
    );

    let _ = request.respond(response);
}

fn handle_active_pane() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let path = match std::env::var("WEZTCODE_ACTIVE_PANE_FILE") {
        Ok(p) => p,
        Err(_) => return json_error("ACTIVE_PANE_FILE not set"),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let pane_id = content.trim().parse::<u32>().unwrap_or(0);
            let data = serde_json::json!({
                "ok": true,
                "data": { "pane_id": pane_id }
            });
            json_response(&data)
        }
        Err(e) => json_error(&format!("Failed to read active_pane: {}", e)),
    }
}

fn handle_terminal_ensure_main() -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let term = WeztermProtocol::new();
    match term.activate_pane(0) {
        Ok(_) => json_response(&serde_json::json!({ "ok": true })),
        Err(e) => json_error(&e),
    }
}

fn main() {
//     println!("WeztCode - Inicializando...");

    if !WeztermProtocol::is_available() {
//         eprintln!("Error: wezterm no está instalado");
        std::process::exit(1);
    }

    // Configure padding hook before spawning terminal
    if let Err(e) = setup_padding_hook() {
//         println!("[Main] Warning: padding hook setup failed: {}", e);
//         println!("[Main] Continuando sin hook de padding...");
    }

    // Create signal channel for toplevel_id capture
    let (capture_signal_tx, capture_signal_rx) = mpsc::channel::<()>();

    let term = WeztermProtocol::new();
    let class = config::WINDOW_CLASS;

//     println!("Iniciando WezTerm...");
    if let Err(e) = term.spawn(class).map(|_| ()) {
//         eprintln!("Error al iniciar terminal: {}", e);
        std::process::exit(1);
    }

    // Start HTTP server first
    let http_port = 8765;
    let _http_thread = start_http_server(http_port);

    // Wait for server to start
    thread::sleep(Duration::from_millis(100));

    let frontend_url = format!("http://127.0.0.1:{}/", http_port);
//     println!("Frontend URL: {}", frontend_url);

    // Detect window manager and setup monitoring FIRST (before creating GTK platform)
    let mut term_geometry = None;
    let mut wm_receiver = None;
    let wm = gui::protocol::wayland::wm::detect_window_manager();

    if let Some(wm) = wm {
//         println!("Window Manager detected: {}", wm.wm_name());

        // Get event receiver from WM (must be called before start_monitoring)
        wm_receiver = Some(wm.event_receiver());

        // Set capture signal channel for toplevel_id capture
        wm.set_capture_signal(capture_signal_rx);

        // Wait for terminal to be ready, then send capture signal
//         println!("[Main] Waiting for terminal to be ready...");
        thread::sleep(Duration::from_millis(1200));

        // Send signal to start toplevel_id capture now that terminal is ready
//         println!("[Main] Sending capture signal to WM thread...");
        let _ = capture_signal_tx.send(());

        // Start monitoring target window - this BLOCKS until initial geometry is captured
        // target_toplevel_id is None initially - it will be captured from the query
//         println!("[Main] Starting window monitoring and waiting for initial geometry...");
        term_geometry = wm.start_monitoring(config::WINDOW_CLASS.to_string(), None);

        if let Some(ref geo) = term_geometry {
//             println!("[Main] Initial geometry captured: x={}, y={}, w={}, h={}",
//                      geo.x, geo.y, geo.width, geo.height);
        } else {
//             println!("[Main] Could not capture initial geometry");
        }
    } else {
//         println!("No se detectó Window Manager - ejecutando en modo standalone");
    }

    // NOW create GUI platform with captured geometry available
    let platform = Gtk4Platform::new();

    // Connect WM events to GUI actions (if WM was detected)
    if let Some(receiver) = wm_receiver {
        platform.handle_wm_events(receiver);
    }

    // Create overlay with captured geometry (or None if no WM)
    if let Err(e) = platform.create_overlay(&frontend_url, term_geometry) {
//         eprintln!("Error al crear overlay: {}", e);
        std::process::exit(1);
    }

    // TODO: technical debt — this creates a visible flicker because spawn_tab()
    // steals focus and activate_pane(0) returns it. Future: use Lua API from a
    // background thread to spawn without auto-focus.
    // Spawn an additional default terminal, then return focus to nvim
    let cwd = crate::config::props::UserProps::load()
        .get("current_dir")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let _term2 = WeztermProtocol::new();
    let _ = _term2.spawn_tab(cwd.as_deref());
    let _ = term.activate_pane(0);

//     println!("WeztCode corriendo...");
    platform.run();
}
