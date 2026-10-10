//! Integración con voxtype (STT por micrófono, daemon externo).
//!
//! Arquitectura (Camino A — daemon + CLI):
//! - `voxtype record start --file=<tmp> --no-osd`: el daemon captura el audio.
//! - `voxtype record stop`: es async (solo señala al daemon), vuelve en ~0.04s.
//! - `voxtype status --follow`: stream de cambios de estado (idle/recording/
//!   transcribing) — se spawnea como hijo de vida corta por cada dictado y un
//!   thread hace blocking-read de su stdout hasta llegar a "idle". Cero polling:
//!   el kernel despierta el thread solo cuando llega una línea.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Timeout de seguridad para la transcripción (dictados largos en hardware lento).
const TRANSCRIBE_TIMEOUT_SECS: u64 = 30;

/// Resuelve el binario voxtype: prop `voxtype_path` en user_props.lua o "voxtype" en PATH.
fn voxtype_binary() -> String {
    let props = crate::config::props::UserProps::load();
    if let Some(path) = props.get("voxtype_path").map(str::trim).filter(|s| !s.is_empty()) {
        return path.to_string();
    }
    "voxtype".to_string()
}

/// Archivo temporal donde el daemon escribe la transcripción.
fn stt_file() -> PathBuf {
    let temp_dir = std::env::var("XDG_RUNTIME_DIR")
        .or_else(|_| std::env::var("TMPDIR"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(temp_dir).join("weztcode-voxtype-stt.txt")
}

/// Inicia la grabación por mic; la transcripción se escribirá en stt_file().
/// Sin pre-check del daemon: `record start` ya reporta un error claro si está
/// caído, y ahorrar ese spawn extra reduce la latencia del click a la mitad.
pub fn record_start() -> Result<(), String> {
    // Limpiar transcripción anterior para no leer texto viejo en un fallo.
    let _ = std::fs::remove_file(stt_file());

    let out = Command::new(voxtype_binary())
        .args(["record", "start", "--no-osd"])
        .arg("--file")
        .arg(stt_file())
        .output()
        .map_err(|e| format!("Failed to run voxtype: {}", e))?;

    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "voxtype record start falló: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Detiene la grabación, espera la transcripción (event-driven) y devuelve el texto.
pub fn record_stop_and_wait() -> Result<String, String> {
    // 1. Señalizar stop (async: vuelve en ~0.04s).
    let out = Command::new(voxtype_binary())
        .args(["record", "stop"])
        .output()
        .map_err(|e| format!("Failed to run voxtype: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "voxtype record stop falló: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    // 2. Follower de vida corta: stream de estados hasta "idle".
    let mut follower = Command::new(voxtype_binary())
        .args(["status", "--follow"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn voxtype status --follow: {}", e))?;

    let reached_idle = wait_for_idle_and_kill(&mut follower);

    if !reached_idle {
        eprintln!("[voxtype] timeout esperando transcripción; best-effort read");
    }

    // 3. Leer la transcripción.
    match std::fs::read_to_string(stt_file()) {
        Ok(text) => Ok(text.trim().to_string()),
        Err(_) => {
            if reached_idle {
                // idle sin archivo: grabación en silencio o VAD filtró todo.
                Ok(String::new())
            } else {
                Err("Transcripción no disponible (timeout o daemon sin responder)".to_string())
            }
        }
    }
}

/// Lee el stream de `status --follow` hasta llegar a "idle", sin polling:
/// un thread bloquea en read_line (el kernel lo despierta con cada línea) y
/// manda cada estado por un channel; el hilo principal espera en
/// recv_timeout (futex). Al salir, mata al hijo ANTES de joinear — si el
/// thread sigue bloqueado en read_line, solo la muerte del hijo (EOF del
/// stdout) lo despierta y evita un deadlock en join().
fn wait_for_idle_and_kill(follower: &mut Child) -> bool {
    let stdout = match follower.stdout.take() {
        Some(s) => s,
        None => return false,
    };

    let (tx, rx) = mpsc::channel::<String>();
    let reader_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break; // receptor cerrado: nadie escucha más
                    }
                }
                Err(_) => break, // EOF o error: el follower murió
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(TRANSCRIBE_TIMEOUT_SECS);
    let mut idle = false;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                if line.trim() == "idle" {
                    idle = true;
                    break;
                }
                // "recording" / "transcribing" → seguir esperando el idle final
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Matar al hijo antes del join (ver doc del método).
    let _ = follower.kill();
    let _ = follower.wait();
    let _ = reader_handle.join();
    idle
}

/// Estado actual del daemon (texto plano: "idle", "stopped", etc.).
pub fn status() -> Result<String, String> {
    let out = Command::new(voxtype_binary())
        .arg("status")
        .output()
        .map_err(|e| format!("Failed to run voxtype: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err("Voxtype daemon no está corriendo".to_string())
    }
}
