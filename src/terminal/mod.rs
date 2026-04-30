use std::process::Child;

pub trait TerminalProtocol {
    /// Spawns a terminal and returns (Child process, PID)
    fn spawn(&self, class: &str) -> Result<(Child, u32), String>;
    fn list_panes(&self) -> Result<String, String>;
    fn send_text(&self, text: &str, pane_id: Option<u32>) -> Result<(), String>;
    fn is_available() -> bool where Self: Sized;
}

pub mod wezterm;

pub use wezterm::WeztermProtocol;
