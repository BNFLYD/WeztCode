# Plan: Decodificar URL en query params del API

## Problema

El frontend envía `fetch("/api/fs/ls?path=/")` y el navegador codifica la URL como `/api/fs/ls?path=%2F`. `parse_query_param()` devuelve `%2F` sin decodificar, y `sanitize_path()` recibe `%2F` en vez de `/`, fallando con "Path not found".

## Solución

Agregar decodificación URL manual en `parse_query_param()` — no necesita dependencias externas.

## Cambio en `src/main.rs`

```rust
fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == key {
            let raw = parts.next().unwrap_or("");
            return Some(url_decode(raw));
        }
    }
    None
}

fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}
```

## Archivo modificado

| Archivo | Cambio |
|---|---|
| `src/main.rs` | Decodificar %XX y + en query params del API |
