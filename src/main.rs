mod api;
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
use std::path::PathBuf;

use axum::Router;
use tower_http::services::ServeDir;

use once_cell::sync::Lazy;

use config::default_terms::DefaultTerm;
use api::chat::set_last_system_prompt;

static CHAT_SERVICE: Lazy<Mutex<chat::ChatService>> = Lazy::new(|| {
    // Check if there's a default sub-agent configured
    let config = if let Some(agent) = config::sub_agents::get_default() {
        set_last_system_prompt(Some(agent.system_prompt.clone()));
        chat::ChatConfig::from_sub_agent(&agent)
    } else {
        chat::ChatConfig::from_default_model()
    };
    let backend: Box<dyn chat::AgentBackend> = Box::new(chat::PiAgentBackend::new(config));
    Mutex::new(chat::ChatService::new(backend))
});

fn setup_padding_hook() -> Result<PathBuf, String> {
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

    Ok(weztcode_dir)
}

fn start_http_server(port: u16) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let app = Router::new()
                .nest("/api", api::router())
                .fallback_service(
                    ServeDir::new("frontend/dist")
                        .append_index_html_on_directories(true)
                );

            let listener = tokio::net::TcpListener::bind(
                format!("127.0.0.1:{}", port)
            ).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    })
}

fn main() {
//     println!("WeztCode - Inicializando...");

    if !WeztermProtocol::is_available() {
//         eprintln!("Error: wezterm no está instalado");
        std::process::exit(1);
    }

    // Configure padding hook before spawning terminal
    let weztcode_dir = match setup_padding_hook() {
        Ok(dir) => dir,
        Err(_) => {
            let temp = if cfg!(target_os = "windows") {
                std::env::var("TEMP").unwrap_or_else(|_| "C:\\Temp".to_string())
            } else {
                std::env::var("XDG_RUNTIME_DIR")
                    .or_else(|_| std::env::var("TMPDIR"))
                    .unwrap_or_else(|_| "/tmp".to_string())
            };
            std::path::PathBuf::from(temp).join("weztcode")
        }
    };

    // Create FIFO for wezterm ready signal (replaces fixed timer)
    let fifo_path = weztcode_dir.join(config::READY_FIFO_NAME);
    let mut has_fifo = true;
    let _ = std::fs::remove_file(&fifo_path);
    if let Err(_e) = nix::unistd::mkfifo(&fifo_path, nix::sys::stat::Mode::S_IRWXU) {
        has_fifo = false;
    }
    if has_fifo {
        std::env::set_var("WEZTCODE_READY_FIFO", fifo_path.to_str().unwrap_or(""));
    }

    // Create signal channel for toplevel_id capture
    let (capture_signal_tx, capture_signal_rx) = mpsc::channel::<()>();

    // Start HTTP server first (concurrente con terminal)
    let http_port = 8765;
    let _http_thread = start_http_server(http_port);

    // Wait for server to start
    thread::sleep(Duration::from_millis(100));

    let term = WeztermProtocol::new();
    let class = config::WINDOW_CLASS;

//     println!("Iniciando WezTerm...");
    if let Err(e) = term.spawn(class).map(|_| ()) {
//         eprintln!("Error al iniciar terminal: {}", e);
        std::process::exit(1);
    }

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

        // Wait for terminal to be ready (via FIFO or fallback timer)
        if has_fifo {
            let (ready_tx, ready_rx) = mpsc::channel::<()>();
            let fifo = fifo_path.clone();
            thread::spawn(move || {
                if let Ok(mut f) = std::fs::File::open(&fifo) {
                    let mut buf = [0u8; 1];
                    f.read_exact(&mut buf).ok();
                }
                let _ = ready_tx.send(());
            });
            match ready_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(_) => {}
                Err(_) => {}
            }
        } else {
            thread::sleep(Duration::from_millis(1200));
        }

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

    // Spawn terminal tabs: from default_terms.json or fallback to generic
    let cwd = crate::config::props::UserProps::load()
        .get("current_dir")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let default_terms = crate::config::default_terms::list();
    let autostart_terms: Vec<DefaultTerm> = default_terms.iter().filter(|t| t.autostart).cloned().collect();
    let term2 = WeztermProtocol::new();
    if autostart_terms.is_empty() {
        let _ = term2.spawn_tab(cwd.as_deref(), None);
    } else {
        for dt in &autostart_terms {
            match term2.spawn_tab(cwd.as_deref(), Some(&dt.program)) {
                Ok(pane_id) => {
                    let _ = crate::config::terms_metadata::set(
                        pane_id,
                        Some(dt.name.clone()),
                        Some(dt.icon.clone()),
                    );
                }
                Err(e) => eprintln!("[main] Failed to spawn '{}': {}", dt.name, e),
            }
        }
    }
    // Always restore focus to pane 0
    let _ = term.activate_pane(0);

    // Create overlay with captured geometry (or None if no WM)
    if let Err(e) = platform.create_overlay(&frontend_url, term_geometry) {
//         eprintln!("Error al crear overlay: {}", e);
        std::process::exit(1);
    }

//     println!("WeztCode corriendo...");
    platform.run();
}
