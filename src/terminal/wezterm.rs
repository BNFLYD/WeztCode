use crate::terminal::TerminalProtocol;
use std::process::{Child, Command, Stdio};

pub struct WeztermProtocol;

impl WeztermProtocol {
    pub fn new() -> Self {
        Self
    }
}

impl TerminalProtocol for WeztermProtocol {
    fn spawn(&self, class: &str) -> Result<(Child, u32), String> {
        let child = Command::new("wezterm")
            .args(["start", "--class", class])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

        let pid = child.id();
        Ok((child, pid))
    }

    fn list_panes(&self) -> Result<String, String> {
        let output = Command::new("wezterm")
            .args(["cli", "list"])
            .output()
            .map_err(|e| format!("Failed to list panes: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn send_text(&self, text: &str, pane_id: Option<u32>) -> Result<(), String> {
        let mut cmd = Command::new("wezterm");
        cmd.arg("cli").arg("send-text").arg(text);

        if let Some(id) = pane_id {
            cmd.arg("--pane-id").arg(id.to_string());
        }

        let output = cmd.output()
            .map_err(|e| format!("Failed to send text: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn is_available() -> bool {
        Command::new("which")
            .arg("wezterm")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
