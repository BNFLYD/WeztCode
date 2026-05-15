# Plan: Comentar logs de Rust (Grupos A + B + C)

## Archivos a modificar

### Grupo A — Core (56 logs)

| Archivo | Logs | Líneas |
|---|---|---|
| `src/main.rs` | 21 | 60-202 |
| `src/gui/gtk4_linux.rs` | 34 | 39-349 |
| `src/terminal/wezterm.rs` | 1 | 16 |

### Grupo B — WM (73 logs)

| Archivo | Logs | Líneas |
|---|---|---|
| `src/.../sway_ipc.rs` | 42 | 85-515 |
| `src/.../foreign_toplevel.rs` | 19 | 105-350 |
| `src/.../wm/mod.rs` | 12 | 64-116 |

### Grupo C — Misc (1 log)

| Archivo | Logs | Línea |
|---|---|---|
| `src/gui/mod.rs` | 1 | 15 |

## Total: 130 logs en 7 archivos

## Método

Anteponer `// ` a cada línea de todo `println!(...)` y `eprintln!(...)`, incluyendo continuaciones multi-línea hasta encontrar `);`.

## No se toca

- `weztcode.lua`
- Cualquier otro archivo fuera de los 7 listados

## Resultado

Zero logs de Rust en la terminal.
