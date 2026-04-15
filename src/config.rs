use once_cell::sync::Lazy;
use std::sync::Mutex;

pub const APP_NAME: &str = "weztcode";
pub const WINDOW_CLASS: &str = "weztcode-terminal";
pub const DEFAULT_UI_WIDTH: i32 = 350;
pub const DEFAULT_UI_HEIGHT: i32 = 600;
pub const FRONTEND_URL: &str = "file:///usr/share/weztcode/index.html";
pub const FRONTEND_DEV_URL: &str = "file://./frontend/dist/index.html";

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
            frontend_path: FRONTEND_DEV_URL.to_string(),
        }
    }
}

pub static GLOBAL_CONFIG: Lazy<Mutex<Config>> = Lazy::new(|| Mutex::new(Config::default()));

pub fn get_config() -> Config {
    GLOBAL_CONFIG.lock().unwrap().clone()
}
