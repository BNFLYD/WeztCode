use std::process::Child;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaneInfo {
    pub tab_id: u32,
    pub pane_id: u32,
    pub title: String,
}

pub trait TerminalProtocol {
    /// Spawns a terminal and returns (Child process, PID)
    fn spawn(&self, class: &str) -> Result<(Child, u32), String>;
    fn list_panes(&self) -> Result<String, String>;
    fn send_text(&self, text: &str, pane_id: Option<u32>) -> Result<(), String>;
    fn set_right_padding(&self, pixels: u32) -> Result<(), String>;
    fn is_available() -> bool where Self: Sized;

    /// Spawns a new tab with an optional program, returns pane_id
    fn spawn_tab(&self, cwd: Option<&str>, program: Option<&str>) -> Result<u32, String>;
    /// Kills a pane by ID
    fn kill_pane(&self, pane_id: u32) -> Result<(), String>;
    /// Activates a pane by ID
    fn activate_pane(&self, pane_id: u32) -> Result<(), String>;

    /// List panes as structured data (parsed from `wezterm cli list`)
    fn list_panes_structured(&self) -> Result<Vec<PaneInfo>, String> {
        let raw = self.list_panes()?;
        let mut panes = Vec::new();
        for line in raw.lines().skip(1) {
            let line = line.trim();
            if line.is_empty() { continue; }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 3 { continue; }
            let tab_id = cols.get(1).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let pane_id = cols.get(2).and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
            let title = cols.get(5..).map(|s| s.join(" ")).unwrap_or_default();
            panes.push(PaneInfo { tab_id, pane_id, title });
        }
        Ok(panes)
    }
}

pub mod lua_spawn;
pub mod wezterm;

pub use wezterm::WeztermProtocol;
