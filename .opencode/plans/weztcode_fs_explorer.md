# Plan: Explorador de archivos vía HTTP API

## Objetivo

Reemplazar los datos mock del `explorer.svelte` por un explorador funcional que liste y lea archivos del sistema real, usando el servidor HTTP existente como backend API.

## Alcance Fase 1 (LIST + READ)

- Listar directorios: archivos y subdirectorios
- Navegar por la estructura de directorios (expandir carpetas)
- Leer contenido de archivos (renderizado en la GUI)
- CRUD completo queda para fase 2

## Cambios

### 1. `src/main.rs` — API routes en el servidor HTTP

Agregar rutas `/api/fs/*` al `start_http_server()`. La lógica se delega a un módulo nuevo.

Estructura actual del handler:

```rust
for request in server.incoming_requests() {
    let url = request.url();
    if url.starts_with("/api/") {
        handle_api(request);
    } else {
        // servir archivos estáticos igual que ahora
    }
}
```

### 2. Nuevo: `src/config/fs.rs` — Lógica de archivos del lado Rust

```rust
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub entry_type: String,  // "file" | "dir"
    pub size: Option<u64>,
    pub modified: Option<String>,
}

pub fn list_dir(path: &str, root: &Path) -> Result<Vec<FsEntry>, String>;
pub fn read_file(path: &str, root: &Path) -> Result<String, String>;
fn sanitize_path(requested: &str, root: &Path) -> Result<PathBuf, String>;
```

- `list_dir`: Lee directorio, devuelve archivos + subdirectorios con nombre, ruta relativa, tipo, tamaño, fecha
- `read_file`: Lee contenido de archivo como string
- `sanitize_path`: Previene path traversal — canonicaliza y verifica que esté dentro del root permitido

### 3. `src/main.rs` — API handler

```rust
fn handle_api(request: tiny_http::Request) {
    let url = request.url().to_string();
    let root = crate::config::props::UserProps::load()
        .get("current_dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let response = if url.starts_with("/api/fs/ls") {
        // GET /api/fs/ls?path=/subdir
        let rel_path = parse_query_param(&url, "path").unwrap_or("/");
        handle_ls(rel_path, &root)
    } else if url.starts_with("/api/fs/read") {
        // GET /api/fs/read?path=/foo/bar.rs
        let rel_path = parse_query_param(&url, "path").unwrap_or("");
        handle_read(rel_path, &root)
    } else {
        json_error("Unknown API endpoint")
    };

    // responder con JSON
    request.respond(response).unwrap();
}
```

### 4. `frontend/src/lib/components/sections/explorer.svelte` — Rewrite completo

Dejar de usar datos mock. El componente:

- Al montarse, fetch a `/api/fs/ls?path=/` para obtener el directorio raíz
- Muestra archivos y carpetas en una lista jerárquica
- Al hacer clic en una carpeta, fetch a `/api/fs/ls?path=/ruta` para expandir
- Al hacer clic en un archivo, fetch a `/api/fs/read?path=/ruta` y muestra el contenido en un panel lateral o modal
- Estados: loading, error, empty

```svelte
<script>
  let current_path = "/";
  let entries = [];
  let loading = true;
  let error = null;
  let selected_file = null;
  let file_content = null;

  async function load_dir(path) {
    loading = true;
    error = null;
    try {
      const res = await fetch(`/api/fs/ls?path=${encodeURIComponent(path)}`);
      const json = await res.json();
      if (json.ok) {
        entries = json.data.files;
        current_path = json.data.path;
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
    loading = false;
  }

  async function read_file(path) {
    try {
      const res = await fetch(`/api/fs/read?path=${encodeURIComponent(path)}`);
      const json = await res.json();
      if (json.ok) {
        file_content = json.data.content;
        selected_file = path;
      }
    } catch (e) {
      // manejar error
    }
  }

  // Cargar raíz al montar
  $effect(() => { load_dir("/"); });
</script>
```

### 5. `user_props.lua` — Nueva variable `current_dir`

```lua
user_editor = "nvim"
current_dir = "/home/usuario/proyecto"  -- opcional, si no existe usa CWD
```

### 6. `src/config/props.rs` — Sin cambios

Ya parsea `user_props.lua` correctamente. Solo se agrega la clave `current_dir` que el usuario puede setear.

## API endpoints

### `GET /api/fs/ls?path=/subdir`

```json
{
  "ok": true,
  "data": {
    "path": "/proyecto",
    "files": [
      { "name": "src", "type": "dir", "size": null, "modified": "2026-05-15T10:00:00Z" },
      { "name": "main.rs", "type": "file", "size": 2048, "modified": "2026-05-14T15:30:00Z" },
      { "name": "README.md", "type": "file", "size": 512, "modified": "2026-05-13T09:00:00Z" }
    ]
  }
}
```

### `GET /api/fs/read?path=/main.rs`

```json
{
  "ok": true,
  "data": {
    "path": "/proyecto/main.rs",
    "content": "fn main() {\n  println!(\"hello\");\n}"
  }
}
```

### Error

```json
{
  "ok": false,
  "error": "Path traversal detected"
}
```

## Seguridad

- `sanitize_path()` canonicaliza la ruta y verifica que esté dentro del root (CWD o `current_dir`)
- Si se intenta `../../etc/passwd`, devuelve error 403
- Solo lectura: no se escribe nada en disco en esta fase

## Archivos nuevos/modificados

| Archivo | Cambio |
|---|---|
| `src/config/fs.rs` | Nuevo: lógica de listar/leer archivos con sanitize |
| `src/main.rs` | + rutas `/api/fs/*` en el servidor HTTP |
| `frontend/src/lib/components/sections/explorer.svelte` | Rewrite: fetch real, tree nav, file viewer |
| `user_props.lua` | + variable `current_dir` opcional |

## No se modifica

- `bridge.js` — se mantiene para futuro, no se usa para archivos
- `wezterm.rs`, `gtk4_linux.rs` — sin cambios
- `weztcode.lua` — sin cambios
