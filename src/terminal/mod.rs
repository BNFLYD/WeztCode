use std::process::Child;

pub trait TerminalProtocol {
    /// Spawns a terminal and returns (Child process, PID)
    fn spawn(&self, class: &str) -> Result<(Child, u32), String>;
    fn list_panes(&self) -> Result<String, String>;
    fn send_text(&self, text: &str, pane_id: Option<u32>) -> Result<(), String>;
    fn set_right_padding(&self, pixels: u32) -> Result<(), String>;
    fn is_available() -> bool where Self: Sized;

    /// Spawns a new tab in the current window, returns pane_id
    fn spawn_tab(&self, cwd: Option<&str>) -> Result<u32, String>;
    /// Kills a pane by ID
    fn kill_pane(&self, pane_id: u32) -> Result<(), String>;
    /// Activates a pane by ID
    fn activate_pane(&self, pane_id: u32) -> Result<(), String>;
}

pub mod wezterm;

pub use wezterm::WeztermProtocol;
