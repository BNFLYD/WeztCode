mod config;
mod gui;
mod terminal;

use gui::{GuiPlatform, Gtk4Platform};
use terminal::{TerminalProtocol, WeztermProtocol};
use std::thread;
use std::time::Duration;
use std::path::PathBuf;
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

    let platform = Gtk4Platform::new();
    let frontend_url = format!("http://127.0.0.1:{}/", http_port);

    println!("Frontend URL: {}", frontend_url);

    if let Err(e) = platform.create_overlay(&frontend_url) {
        eprintln!("Error al crear overlay: {}", e);
        std::process::exit(1);
    }

    println!("WeztCode corriendo...");
    platform.run();
}
