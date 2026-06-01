use crate::config::default_terms::DefaultTerm;
use crate::terminal::wezterm::{run_cmd_with_timeout, WeztermProtocol};
use std::process::Command;
use std::time::Duration;

fn build_lua_script(terms: &[DefaultTerm], cwd: Option<&str>) -> String {
    let mut script = String::from(
        "local wezterm = wezterm or require(\"wezterm\")\n\
         local mux = wezterm.mux\n\
         local results = {}\n\n\
         local function split(str)\n\
           local parts = {}\n\
           for part in str:gmatch(\"%S+\") do\n\
             table.insert(parts, part)\n\
           end\n\
           return parts\n\
         end\n\n\
         local function spawn_tab(program, cwd)\n\
           local ok, tab, pane, window = pcall(function()\n\
             return mux.spawn_tab({\n\
               args = split(program),\n\
               cwd = cwd ~= \"\" and cwd or nil,\n\
             })\n\
           end)\n\
           if ok and pane then\n\
             table.insert(results, pane:pane_id())\n\
           else\n\
             table.insert(results, -1)\n\
           end\n\
         end\n\n"
    );

    let cwd_str = cwd.unwrap_or("");
    for term in terms {
        script.push_str(&format!(
            "spawn_tab({:?}, {:?})\n",
            term.program, cwd_str
        ));
    }

    script.push_str(
        "\n-- Build JSON manually\n\
         local parts = {}\n\
         for i, id in ipairs(results) do\n\
           parts[i] = tostring(id)\n\
         end\n\
         local json = \"[\" .. table.concat(parts, \",\") .. \"]\"\n\
         local f = io.open(\""
    );
    script.push_str(&get_out_path().to_string_lossy().replace("\\", "/"));
    script.push_str(
        "\", \"w\")\n\
         if f then\n\
           f:write(json)\n\
           f:close()\n\
         end\n\
         return 12\n"
    );

    script
}

fn get_out_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("weztcode_spawn_out.json");
    path
}

fn get_lua_path() -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push("weztcode_spawn.lua");
    path
}

pub fn spawn_autostart_terms(terms: &[DefaultTerm], cwd: Option<&str>) -> Result<Vec<(u32, String, String)>, String> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    // Write Lua script to temp file
    let lua_script = build_lua_script(terms, cwd);
    let lua_path = get_lua_path();
    std::fs::write(&lua_path, &lua_script)
        .map_err(|e| format!("Failed to write Lua script: {}", e))?;

    // Remove old output file
    let out_path = get_out_path();
    let _ = std::fs::remove_file(&out_path);

    // Execute via wezterm --config to inject Lua into the running instance
    // `start --new-tab` connects to the existing mux, the --config Lua is evaluated
    //   before the new tab opens, spawning autostart terminals via mux API.
    // The new-tab creates one extra empty shell tab as a side effect.
    let mut cmd = Command::new("wezterm");
    cmd.args([
        "--config",
        &format!("_=dofile(\"{}\")", lua_path.to_string_lossy().replace("\\", "/")),
        "start",
        "--new-tab",
        "--class",
        crate::config::WINDOW_CLASS,
    ]);

    let _ = run_cmd_with_timeout(&mut cmd, Duration::from_secs(10));

    // Give wezterm time to process
    std::thread::sleep(Duration::from_millis(500));

    // Try to read output file
    if let Ok(content) = std::fs::read_to_string(&out_path) {
        if let Ok(ids) = serde_json::from_str::<Vec<i64>>(&content) {
            if ids.len() == terms.len() && ids.iter().all(|id| *id > 0) {
                let results: Vec<(u32, String, String)> = ids.iter()
                    .zip(terms.iter())
                    .map(|(pane_id, term)| (*pane_id as u32, term.name.clone(), term.icon.clone()))
                    .collect();
                return Ok(results);
            }
        }
    }

    Err("Lua spawn did not produce valid output".to_string())
}

pub fn spawn_autostart_terms_fallback(terms: &[DefaultTerm], cwd: Option<&str>) -> Result<Vec<(u32, String, String)>, String> {
    let term = WeztermProtocol::new();
    let mut results = Vec::new();

    for dt in terms {
        match term.spawn_tab(cwd, Some(&dt.program)) {
            Ok(pane_id) => {
                results.push((pane_id, dt.name.clone(), dt.icon.clone()));
            }
            Err(e) => {
                eprintln!("[lua_spawn] Fallback spawn failed for '{}': {}", dt.name, e);
            }
        }
    }

    Ok(results)
}
