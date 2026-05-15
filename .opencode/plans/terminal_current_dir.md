# Plan: Aplicar current_dir al editor de la terminal

## Problema

`spawn()` lanza `wezterm start --class <class> nvim`, pero nvim abre en el directorio por defecto de WezTerm, no en `current_dir` definido en `user_props.lua`.

## Solución

Pasar `--cwd <dir>` a `wezterm start` para que el editor inicie en el directorio del proyecto.

## Cambio

### `src/terminal/wezterm.rs` — `spawn()`

```rust
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
    }

    let child = cmd
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to spawn wezterm: {}", e))?;

    let pid = child.id();
    Ok((child, pid))
}
```

## Archivo modificado

| Archivo | Cambio |
|---|---|
| `src/terminal/wezterm.rs` | + `--cwd` con `current_dir` de `user_props.lua` |
