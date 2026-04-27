mod config;
mod gui;
mod terminal;

use gui::{GuiPlatform, Gtk4Platform};
use terminal::{TerminalProtocol, WeztermProtocol};
use gtk4::glib;
use std::thread;
use std::time::Duration;

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

    let term = WeztermProtocol::new();
    let class = config::WINDOW_CLASS;

    println!("Iniciando WezTerm...");
    match term.spawn(class) {
        Ok(_) => println!("WezTerm iniciado"),
        Err(e) => {
            eprintln!("Error al iniciar terminal: {}", e);
            std::process::exit(1);
        }
    }

    thread::sleep(Duration::from_millis(1200));

    // Iniciar servidor HTTP
    let http_port = 8765;
    let _http_thread = start_http_server(http_port);

    // Esperar a que el servidor inicie
    thread::sleep(Duration::from_millis(100));

    // Detectar window manager y obtener geometría de la terminal
    let wm = gui::protocol::wayland::wm::detect_window_manager();
    let term_geometry = wm.as_ref().and_then(|wm| wm.get_window_geometry("weztcode"));

    if let Some(geo) = &term_geometry {
        println!("Geometría de terminal detectada: x={}, y={}, w={}, h={}",
                 geo.x, geo.y, geo.width, geo.height);
    } else {
        println!("Usando geometría por defecto");
    }

    let platform = Gtk4Platform::new();
    let frontend_url = format!("http://127.0.0.1:{}/", http_port);

    println!("Frontend URL: {}", frontend_url);

    if let Err(e) = platform.create_overlay(&frontend_url, term_geometry) {
        eprintln!("Error al crear overlay: {}", e);
        std::process::exit(1);
    }

    // Configurar callback de foco para ocultar/mostrar automáticamente
    if let Some(wm) = wm {
        // Crear canal para comunicación thread-safe con GTK
        let (sender, receiver) = glib::MainContext::channel(glib::Priority::DEFAULT);

        // Conectar callback de foco (ejecuta en thread de Wayland)
        if let Err(e) = wm.on_focus_change("weztcode", Box::new(move |focused| {
            let _ = sender.send(focused);
        })) {
            eprintln!("Error al registrar callback de foco: {}", e);
        }

        // Conectar receptor al main context de GTK (ejecuta en main thread)
        receiver.attach(None, move |focused| {
            if focused {
                println!("WezTerm enfocada - mostrando overlay");
                platform.show();
            } else {
                println!("WezTerm perdió foco - ocultando overlay");
                platform.hide();
            }
            glib::ControlFlow::Continue
        });
    }

    println!("WeztCode corriendo...");
    platform.run();
}
