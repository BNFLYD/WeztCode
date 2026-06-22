use crate::terminal::TerminalProtocol;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

pub struct WeztermProtocol;

pub fn run_cmd_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<std::process::Output, String> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    // Take ownership of pipes before the polling loop to read concurrently
    // and avoid pipe-buffer deadlock (child blocks on write if nobody reads)
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_handle = stdout_pipe.map(|mut pipe| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    });

    let stderr_handle = stderr_pipe.map(|mut pipe| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    });

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

    let stdout = stdout_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();
    let stderr = stderr_handle
        .map(|h| h.join().unwrap_or_default())
        .unwrap_or_default();

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
        let current_dir = {
            let root = crate::config::current_root::get();
            let s = root.to_string_lossy().to_string();
            if s.is_empty() { None } else { Some(s) }
        };

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

        let result = try_list(true);
        match result {
            Ok(s) if !s.trim().is_empty() => Ok(s),
            Ok(_) => {
                eprintln!("[list_panes] empty with --class, retrying without");
                try_list(false)
            }
            Err(e) => {
                eprintln!("[list_panes] error with --class ({}), retrying without", e.trim());
                try_list(false)
            }
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
