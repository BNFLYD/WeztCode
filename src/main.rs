mod config;
mod gui;
mod terminal;

use gui::{GuiPlatform, Gtk4Platform};
use terminal::{TerminalProtocol, WeztermProtocol};
use std::thread;
use std::time::Duration;

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

    let platform = Gtk4Platform::new();
    let frontend_url = config::get_config().frontend_path;

    println!("Frontend URL: {}", frontend_url);

    if let Err(e) = platform.create_overlay(&frontend_url) {
        eprintln!("Error al crear overlay: {}", e);
        std::process::exit(1);
    }

    println!("WeztCode corriendo...");
    platform.run();
}
