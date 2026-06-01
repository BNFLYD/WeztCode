use crate::terminal::TerminalProtocol;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct WeztermProtocol;

pub fn run_cmd_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<std::process::Output, String> {
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn: {}", e))?;

    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    child.wait().ok();
                    return Err("Command timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                let _ = child.kill();
                return Err(format!("Failed to wait: {}", e));
            }
        }
    };

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(ref mut out) = child.stdout {
        let _ = out.read_to_end(&mut stdout);
    }
    if let Some(ref mut err) = child.stderr {
        let _ = err.read_to_end(&mut stderr);
    }

    Ok(std::process::Output { status, stdout, stderr })
}

impl WeztermProtocol {
    pub fn new() -> Self {
        Self
    }
}

impl TerminalProtocol for WeztermProtocol {
    fn set_right_padding(&self, pixels: u32) -> Result<(), String> {
        let pad_path = std::env::var("WEZTCODE_PAD_FILE")
            .map_err(|_| "WEZTCODE_PAD_FILE not set".to_string())?;
// Back up
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
        let class = crate::config::WINDOW_CLASS;

        let try_list = |use_class: bool| -> Result<String, String> {
            let mut cmd = Command::new("wezterm");
            cmd.arg("cli");
            if use_class {
                cmd.args(["--class", class]);
            }
            cmd.arg("list");
            let output = run_cmd_with_timeout(&mut cmd, Duration::from_secs(5))?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        };

        let result = try_list(true)?;
        if result.trim().is_empty() {
            eprintln!("[list_panes] empty with --class, retrying without");
            try_list(false)
        } else {
            Ok(result)
        }
    }

    fn send_text(&self, text: &str, pane_id: Option<u32>) -> Result<(), String> {
        let mut cmd = Command::new("wezterm");
        cmd.arg("cli").arg("send-text").arg(text);

        if let Some(id) = pane_id {
            cmd.arg("--pane-id").arg(id.to_string());
        }

        let output = run_cmd_with_timeout(&mut cmd, Duration::from_secs(5))?;

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

    fn spawn_tab(&self, cwd: Option<&str>, program: Option<&str>) -> Result<u32, String> {
        let mut cmd = Command::new("wezterm");
        cmd.args(["cli", "--class", crate::config::WINDOW_CLASS, "spawn"]);

        if let Some(dir) = cwd {
            cmd.arg("--cwd").arg(dir);
        }

        if let Some(prog) = program {
            cmd.arg("--");
            for arg in prog.split_whitespace() {
                cmd.arg(arg);
            }
        }

        let output = run_cmd_with_timeout(&mut cmd, Duration::from_secs(5))?;

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
        let mut cmd = Command::new("wezterm");
        cmd.args(["cli", "--class", crate::config::WINDOW_CLASS, "kill-pane", "--pane-id", &pane_id.to_string()]);
        let output = run_cmd_with_timeout(&mut cmd, Duration::from_secs(5))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    fn activate_pane(&self, pane_id: u32) -> Result<(), String> {
        let mut cmd = Command::new("wezterm");
        cmd.args(["cli", "--class", crate::config::WINDOW_CLASS, "activate-pane", "--pane-id", &pane_id.to_string()]);
        let output = run_cmd_with_timeout(&mut cmd, Duration::from_secs(5))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
