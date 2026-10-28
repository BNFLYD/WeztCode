//! Puertos (sección 表示): lista de servidores escuchando en localhost y
//! control de la ventana viewer (layer-shell al lado del menú).

use axum::{extract::Json, response::IntoResponse};

use crate::api::{err_json, ok_json};

#[derive(serde::Serialize)]
struct PortEntry {
    port: u16,
    process: String,
}

/// Escanea puertos TCP en LISTEN en localhost via `ss -tlnpH`.
/// Solo >= 1024 (excluye servicios del sistema) y != 8765 (nuestro propio server).
fn scan() -> Vec<PortEntry> {
    let out = match std::process::Command::new("ss").args(["-tlnpH"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut seen = std::collections::HashSet::new();
    let mut ports = Vec::new();

    for line in stdout.lines() {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 || cols[0] != "LISTEN" {
            continue;
        }
        // cols[3] = dirección:puerto (ej: 127.0.0.1:5173, [::1]:3000, *:8080)
        let local = cols[3];
        let Some((host, port_s)) = local.rsplit_once(':') else { continue };
        let Ok(port) = port_s.parse::<u16>() else { continue };
        if port < 1024 || port == 8765 {
            continue;
        }
        // Excluir IPs de red externas concretas (solo localhost/wildcard)
        if host.starts_with("192.168.")
            || host.starts_with("10.")
            || host.starts_with("172.")
            || host.starts_with("169.254.")
        {
            continue;
        }
        if !seen.insert(port) {
            continue; // IPv4/IPv6 duplicados
        }

        // Proceso: users:(("vite",pid=1234,fd=20))
        let process = cols
            .get(4)
            .and_then(|c| c.split("(\"").nth(1))
            .and_then(|s| s.split("\",").next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("?")
            .to_string();

        ports.push(PortEntry { port, process });
    }

    ports.sort_by_key(|p| p.port);
    ports
}

/// GET /ports/list — puertos escuchando con su proceso.
pub async fn handle_ports_list() -> impl IntoResponse {
    let ports = tokio::task::spawn_blocking(scan).await.unwrap();
    ok_json(serde_json::json!({"ok": true, "data": ports}))
}

/// POST /ports/open {url} — abre/reemplaza la ventana viewer en esa URL.
pub async fn handle_port_open(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    let url = match body.get("url").and_then(|v| v.as_str()) {
        Some(u) => u.trim().to_string(),
        None => return err_json("Falta el campo 'url'"),
    };
    // Frontera de confianza: solo http(s) (nada de file:// ni javascript:).
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return err_json("URL inválida: debe empezar con http:// o https://");
    }
    match crate::gui::viewer::open_url(url.clone()) {
        Ok(()) => ok_json(serde_json::json!({"ok": true, "data": {"url": url}})),
        Err(e) => err_json(&e),
    }
}

/// POST /ports/close — cierra la ventana viewer.
pub async fn handle_port_close() -> impl IntoResponse {
    match crate::gui::viewer::close_port() {
        Ok(()) => ok_json(serde_json::json!({"ok": true})),
        Err(e) => err_json(&e),
    }
}

/// GET /ports/status — URL que el viewer está mostrando (null = cerrada).
pub async fn handle_port_status() -> impl IntoResponse {
    ok_json(serde_json::json!({"ok": true, "data": {"url": crate::gui::viewer::status()}}))
}
