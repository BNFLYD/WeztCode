use crate::terminal::TerminalProtocol;
use std::process::{Child, Command, Stdio};

pub struct WeztermProtocol;

impl WeztermProtocol {
    pub fn new() -> Self {
        Self
    }
}

impl TerminalProtocol for WeztermProtocol {
    fn set_right_padding(&self, pixels: u32) -> Result<(), String> {
        let pad_path = std::env::var("WEZTCODE_PAD_FILE")
            .map_err(|_| "WEZTCODE_PAD_FILE not set".to_string())?;
//         println!("[WezTerm] set_right_padding: writing {} to {}", pixels, pad_path);
        std::fs::write(&pad_path, pixels.to_string())
            .map_err(|e| format!("Failed to write padding file: {}", e))
    }

    fn spawn(&self, class: &str) -> Result<(Child, u32), String> {
        let props = crate::config::props::UserProps::load();

        let editor = props.get("user_editor").map(|s| s.to_string());
        let current_dir = props.get("current_dir")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let mut cmd = Command::new("wezterm");
        cmd.arg("start").arg("--class").arg(class);

        if let Some(ref dir) = current_dir {
            cmd.arg("--cwd").arg(dir);
        }

        if let Some(ref prog) = editor {
            cmd.arg(prog);
            cmd.arg("--listen").arg("/tmp/weztcode-nvim.sock");
        }

        let child = cmd
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

    fn spawn_tab(&self, cwd: Option<&str>) -> Result<u32, String> {
        let mut cmd = Command::new("wezterm");
        cmd.args(["cli", "spawn"]);
        if let Ok(window_id) = std::env::var("WEZTCODE_WINDOW_ID") {
            cmd.arg("--window-id").arg(window_id);
        }
        if let Some(dir) = cwd {
            cmd.arg("--cwd").arg(dir);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("Failed to spawn tab: {}", e))?;

        if output.status.success() {
            let pane_id = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
                .map_err(|e| format!("Failed to parse pane_id: {}", e))?;
            Ok(pane_id)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn kill_pane(&self, pane_id: u32) -> Result<(), String> {
        let output = Command::new("wezterm")
            .args(["cli", "kill-pane", "--pane-id", &pane_id.to_string()])
            .output()
            .map_err(|e| format!("Failed to kill pane: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn activate_pane(&self, pane_id: u32) -> Result<(), String> {
        let output = Command::new("wezterm")
            .args(["cli", "activate-pane", "--pane-id", &pane_id.to_string()])
            .output()
            .map_err(|e| format!("Failed to activate pane: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
