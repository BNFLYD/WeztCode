mod config;
mod gui;
mod terminal;

use gui::{GuiPlatform, Gtk4Platform};
use terminal::{TerminalProtocol, WeztermProtocol};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::fs::read_to_string;

fn start_http_server(port: u16) -> thread::JoinHandle<()> {
    let server = tiny_http::Server::http(format!("127.0.0.1:{}", port)).unwrap();
    println!("HTTP Server iniciado en http://127.0.0.1:{}/", port);

    thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url();
            let path = if url == "/" {
                "frontend/dist/index.html"
            } else {
                &format!("frontend/dist{}", url)
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

            match read_to_string(path) {
                Ok(content) => {
                    let response = tiny_http::Response::from_string(content)
                        .with_header(tiny_http::Header {
                            field: "Content-Type".parse().unwrap(),
                            value: content_type.parse().unwrap(),
                        });
                    request.respond(response).unwrap();
                }
                Err(_) => {
                    let response = tiny_http::Response::from_string("Not found")
                        .with_status_code(404);
                    request.respond(response).unwrap();
                }
            }
        }
    })
}

fn main() {
    println!("WeztCode - Inicializando...");

    if !WeztermProtocol::is_available() {
        eprintln!("Error: wezterm no está instalado");
        std::process::exit(1);
    }

    // Create signal channel for toplevel_id capture
    let (capture_signal_tx, capture_signal_rx) = mpsc::channel::<()>();

    let term = WeztermProtocol::new();
    let class = config::WINDOW_CLASS;

    println!("Iniciando WezTerm...");
    let target_pid = match term.spawn(class) {
        Ok((_child, pid)) => {
            println!("[Main] WezTerm iniciado con PID: {}", pid);
            pid
        }
        Err(e) => {
            eprintln!("Error al iniciar terminal: {}", e);
            std::process::exit(1);
        }
    };

    thread::sleep(Duration::from_millis(1200));

    // Send signal to start toplevel_id capture now that terminal is ready
    println!("[Main] Sending capture signal to WM thread...");
    let _ = capture_signal_tx.send(());

    //  Start HTTP server
    let http_port = 8765;
    let _http_thread = start_http_server(http_port);

    // Wait for server to start
    thread::sleep(Duration::from_millis(100));

    // Detect window manager and get terminal geometry
    let wm = gui::protocol::wayland::wm::detect_window_manager();
    let term_geometry = wm.as_ref().and_then(|wm| wm.get_window_geometry(config::WINDOW_CLASS));

    if let Some(geo) = &term_geometry {
        println!("Geometría de terminal detectada: x={}, y={}, w={}, h={}",
                 geo.x, geo.y, geo.width, geo.height);
    } else {
        println!("Using default geometry");
    }

    // Initialize GUI platform
    let platform = Gtk4Platform::new();
    let frontend_url = format!("http://127.0.0.1:{}/", http_port);

    println!("Frontend URL: {}", frontend_url);

    // Configure window manager if available
    if let Some(wm) = wm {
        println!("Window Manager detected: {}", wm.wm_name());

        // Set capture signal channel for toplevel_id capture
        wm.set_capture_signal(capture_signal_rx);

        // Get event receiver from WM
        let receiver = wm.event_receiver();

        // Connect WM events to GUI actions
        platform.handle_wm_events(receiver);

        // Start monitoring target window
        // target_toplevel_id is None initially - it will be captured from the first matching event
        wm.start_monitoring(config::WINDOW_CLASS.to_string(), None);
    } else {
        println!("No se detectó Window Manager - ejecutando en modo standalone");
    }

    if let Err(e) = platform.create_overlay(&frontend_url, term_geometry) {
        eprintln!("Error al crear overlay: {}", e);
        std::process::exit(1);
    }

    println!("WeztCode corriendo...");
    platform.run();
}
