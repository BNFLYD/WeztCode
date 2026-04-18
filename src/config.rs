use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::env;
use std::path::PathBuf;

pub const APP_NAME: &str = "weztcode";
pub const WINDOW_CLASS: &str = "weztcode-terminal";
pub const DEFAULT_UI_WIDTH: i32 = 350;
pub const DEFAULT_UI_HEIGHT: i32 = 600;
pub const FRONTEND_URL: &str = "file:///usr/share/weztcode/index.html";

pub fn get_frontend_path() -> String {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let frontend_path = current_dir.join("frontend/dist/index.html");
    format!("file://{}", frontend_path.to_string_lossy())
}

#[derive(Debug, Clone)]
pub struct Config {
    pub terminal_class: String,
    pub ui_width: i32,
    pub ui_height: i32,
    pub frontend_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            terminal_class: WINDOW_CLASS.to_string(),
            ui_width: DEFAULT_UI_WIDTH,
            ui_height: DEFAULT_UI_HEIGHT,
            frontend_path: get_frontend_path(),
        }
    }
}

pub static GLOBAL_CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::default()));

pub fn get_config() -> Config {
    GLOBAL_CONFIG.lock().unwrap().clone()
}
