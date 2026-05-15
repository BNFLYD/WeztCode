# Plan: Abrir archivo en editor desde el explorador

## Objetivo

Al hacer clic en un archivo en la GUI del explorador, el editor (nvim) en WezTerm abre ese archivo.

## Flujo

```
Frontend (clic en archivo)
  → fetch GET /api/editor/open?path=/src/main.rs
  → Rust: construye ruta absoluta
  → Rust: wezterm cli send-text ":e /proyecto/src/main.rs\r"
  → WezTerm recibe el texto
  → nvim ejecuta :e /proyecto/src/main.rs
```

## Cambios

### 1. `src/main.rs` — Nuevo endpoint `/api/editor/open`

```rust
} else if url.starts_with("/api/editor/open") {
    let rel_path = parse_query_param(url, "path").unwrap_or_else(|| String::new());
    handle_editor_open(&rel_path, &root)
}
```

### 2. `src/main.rs` — handler `handle_editor_open()`

```rust
fn handle_editor_open(rel_path: &str, root: &Path) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    // Sanitizar path
    let full_path = match crate::config::fs::sanitize_path(rel_path, root) {
        Ok(p) => p,
        Err(e) => return json_error(&e),
    };

    // Verificar que existe y es archivo
    if !full_path.is_file() {
        return json_error("Not a file");
    }

    // Construir comando :e <path> con Enter al final
    let cmd = format!(":e {}\r", full_path.to_string_lossy());

    // Enviar a WezTerm via cli
    let output = std::process::Command::new("wezterm")
        .args(["cli", "send-text", "--no-paste", &cmd])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            json_response(&serde_json::json!({ "ok": true }))
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            json_error(&format!("send-text failed: {}", stderr))
        }
        Err(e) => json_error(&format!("Failed to run wezterm cli: {}", e)),
    }
}
```

**Nota**: `--no-paste` evita que WezTerm bracketee el texto como pegado, necesario para que nvim lo interprete como comando.

**Nota 2**: Si el editor no es nvim, se puede cambiar `:e ` por otro comando en el futuro (`code -g`, `nano +LINE`, etc.).

### 3. `src/config/fs.rs` — Hacer `sanitize_path()` pública

Cambiar `fn sanitize_path(...)` a `pub fn sanitize_path(...)`.

### 4. `frontend/src/lib/components/sections/explorer.svelte` — Agregar clic en archivos

```svelte
<!-- En el bloque de archivos, dentro de la lista -->
<button
  class="flex items-center gap-2 flex-1 text-left"
  on:click={() => open_file(entry.path)}
>
  <span class="text-print/70">
    <Icon icon={file_icon(entry.name)} class="w-4 h-4" />
  </span>
  <span class="text-print/70 flex-1 truncate">{entry.name}</span>
  {#if entry.size}
    <span class="text-print/30 text-xs">{Math.round(entry.size / 1024)}KB</span>
  {/if}
</button>
```

Y en el script:

```svelte
async function open_file(path) {
  await fetch(`/api/editor/open?path=${encodeURIComponent(path)}`);
}
```

## Archivos modificados

| Archivo | Cambio |
|---|---|
| `src/main.rs` | + endpoint `/api/editor/open`, handler `handle_editor_open()` |
| `src/config/fs.rs` | `sanitize_path()` de `fn` a `pub fn` |
| `frontend/.../explorer.svelte` | Clic en archivo → fetch a `/api/editor/open` |

## No se modifica

- `wezterm.rs` — `send_text()` ya existe y funciona
- `weztcode.lua`, `user_props.lua` — sin cambios
