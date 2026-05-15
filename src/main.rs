mod config;
mod gui;
mod terminal;

use gui::{GuiPlatform, Gtk4Platform};
use terminal::{TerminalProtocol, WeztermProtocol};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

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

    std::env::set_var("WEZTCODE_SESSION", "true");
    std::env::set_var("WEZTERM_CONFIG_FILE", lua_str);
    std::env::set_var("WEZTCODE_PAD_FILE", pad_str);

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
    let root = crate::config::props::UserProps::load()
        .get("current_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let response: tiny_http::Response<std::io::Cursor<Vec<u8>>> = if url.starts_with("/api/fs/ls") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| "/".to_string());
        handle_ls(&rel_path, &root)
    } else if url.starts_with("/api/fs/read") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_read(&rel_path, &root)
    } else if url.starts_with("/api/editor/open") {
        let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
        handle_editor_open(&rel_path, &root)
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
    macro_rules! log_msg {
        ($($arg:tt)*) => {{
            use std::io::Write;
            let _ = writeln!(std::io::stdout(), "{}", format_args!($($arg)*));
            let _ = std::io::stdout().flush();
        }};
    }

    log_msg!("[editor_open] rel_path={:?}, root={:?}", rel_path, root);

    let full_path = match crate::config::fs::sanitize_path(rel_path, root) {
        Ok(p) => p,
        Err(e) => {
            log_msg!("[editor_open] sanitize_path error: {}", e);
            return json_error(&e);
        }
    };

    if !full_path.is_file() {
        log_msg!("[editor_open] not a file: {:?}", full_path);
        return json_error("Not a file");
    }

    log_msg!("[editor_open] opening file: {:?}", full_path);

    match std::process::Command::new("nvim")
        .args([
            "--server", "/tmp/weztcode-nvim.sock",
            "--remote", &full_path.to_string_lossy()
        ])
        .output()
    {
        Ok(o) if o.status.success() => {
            log_msg!("[editor_open] success");
            json_response(&serde_json::json!({ "ok": true }))
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            log_msg!("[editor_open] nvim failed: {}", stderr);
            json_error(&format!("nvim --remote failed: {}", stderr))
        }
        Err(e) => {
            log_msg!("[editor_open] failed to run nvim: {}", e);
            json_error(&format!("Failed to run nvim: {}", e))
        }
    }
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
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
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
    let target_pid = match term.spawn(class) {
        Ok((_child, pid)) => {
//             println!("[Main] WezTerm iniciado con PID: {}", pid);
            pid
        }
        Err(e) => {
//             eprintln!("Error al iniciar terminal: {}", e);
            std::process::exit(1);
        }
    };

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

//     println!("WeztCode corriendo...");
    platform.run();
}
