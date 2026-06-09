-- WeztCode padding hook - parte del binario, no modificar
local wezterm = require "wezterm"
wezterm.log_info("weztcode.lua loaded")

-- 1. Heredar config del usuario
local user_config_path = os.getenv("WEZTCODE_USER_CONFIG")
local user_config = {}

if user_config_path then
  local ok, result = pcall(dofile, user_config_path)
  if ok then
    user_config = result or {}
    wezterm.log_info("User config loaded successfully")
  else
    wezterm.log_info("Failed to load user config: " .. tostring(result))
  end
end

-- 2. Leer side_pad para padding dinámico
local pad_file = os.getenv("WEZTCODE_PAD_FILE")
local gui_padding = 0

if pad_file then
  local f = io.open(pad_file, "r")
  if f then
    gui_padding = tonumber(f:read("*a")) or 0
    f:close()
  end
end

wezterm.log_info("GUI_PADDING from side_pad = " .. gui_padding)

-- 3. Aplicar padding si hay valor válido (se ejecuta en reload-config)
local pad_applied = gui_padding > 0

if pad_applied then
  local base = user_config.window_padding or {}
  user_config.window_padding = {
    left   = base.left,
    right  = (base.right or 0) + gui_padding,
    top    = base.top,
    bottom = base.bottom,
  }
  wezterm.log_info("Padding applied via config reload")
end

-- 4. update-status: reintentar si aún no se pudo aplicar
wezterm.on("update-status", function(window, pane)
  if pad_applied then return end

  local is_weztcode = os.getenv("WEZTCODE_SESSION") == "true"
  if not is_weztcode then return end

  local retry_pad = 0
  if pad_file then
    local f = io.open(pad_file, "r")
    if f then
      retry_pad = tonumber(f:read("*a")) or 0
      f:close()
    end
  end

  if retry_pad > 0 then
    pad_applied = true
    wezterm.log_info("GUI_PADDING from side_pad (retry) = " .. retry_pad)

    local base = user_config.window_padding or {}
    local padding = {
      left   = base.left,
      right  = (base.right or 0) + retry_pad,
      top    = base.top,
      bottom = base.bottom,
    }
    local overrides = window:get_config_overrides() or {}
    overrides.window_padding = padding
    window:set_config_overrides(overrides)
    wezterm.log_info("Padding applied via update-status")
  end
end)

-- Track the last non-zero pane for toggling to nvim
local last_non_zero_pane = nil
local last_written_pane = nil

wezterm.on("update-status", function(window, pane)
    if pane then
        local id = pane:pane_id()

        if id ~= 0 then
            last_non_zero_pane = id
        end

        if id ~= last_written_pane then
            last_written_pane = id
            local active_pane_file = os.getenv("WEZTCODE_ACTIVE_PANE_FILE")
            if active_pane_file then
                local f = io.open(active_pane_file, "w")
                if f then
                    f:write(tostring(id))
                    f:close()
                end
            end
        end
    end
end)

-- Signal readiness via FIFO for faster GTK init (no polling)
wezterm.on("gui-startup", function()
    local fifo = os.getenv("WEZTCODE_READY_FIFO")
    if fifo then
        local ok, err = pcall(function()
            local f = io.open(fifo, "w")
            if f then
                f:write("1")
                f:close()
            end
        end)
        if not ok then
            wezterm.log_warn("failed to write ready fifo: " .. tostring(err))
        end
    end
end)

-- Ocultar completamente la barra de tabs (WeztCode maneja su propia UI)
user_config.enable_tab_bar = false

-- 5. Keybindings de navegación entre tabs
local function load_keybindings()
    local bindings = {
        tab_next = "CTRL+J",
        tab_prev = "CTRL+K",
    }
    local props_path = os.getenv("WEZTCODE_PROPS_FILE")
    if props_path then
        local f = io.open(props_path, "r")
        if f then
            local content = f:read("*a")
            f:close()
            for line in content:gmatch("[^\r\n]+") do
                local trimmed = line:match("^%s*(.-)%s*$")
                if trimmed and not trimmed:match("^%-%-") and trimmed:match("=") then
                    local k, v = trimmed:match("^(%w+)%s*=%s*\"([^\"]+)\"")
                    if k and v then
                        bindings[k] = v
                    end
                end
            end
        end
    end
    return bindings
end

local function parse_key(key_combo)
    local mods_str, key = key_combo:match("^(.+)%+(%w)$")
    if not mods_str then
        mods_str, key = "", key_combo:match("^(%w)$")
    end
    return key and key:lower(), mods_str:upper()
end

local keys = load_keybindings()
wezterm.log_info("Keybindings: tab_next=" .. keys.tab_next)

user_config.keys = user_config.keys or {}

local k, m = parse_key(keys.tab_next)
if k then
    table.insert(user_config.keys, {
        key = k,
        mods = m,
        action = wezterm.action_callback(function(window, pane)
            local active = window:active_pane()
            if not active then return end

            local active_id = active:pane_id()

            if active_id == 0 then
                local target_id = last_non_zero_pane
                if not target_id then
                    -- Fallback: find the highest pane_id (most recently created)
                    local windows = wezterm.mux.all_windows()
                    for _, win in ipairs(windows) do
                        local tabs = win:tabs()
                        for _, tab in ipairs(tabs) do
                            local panes = tab:panes()
                            for _, p in ipairs(panes) do
                                local pid = p:pane_id()
                                if pid ~= 0 and (not target_id or pid > target_id) then
                                    target_id = pid
                                end
                            end
                        end
                    end
                    if target_id then
                        wezterm.log_info("toggle: fallback target_id = " .. target_id)
                    end
                end
                if target_id then
                    local target, _, _ = wezterm.mux.get_pane(target_id)
                    if target then target:activate() end
                end
            else
                last_non_zero_pane = active_id
                local target, _, _ = wezterm.mux.get_pane(0)
                if target then target:activate() end
            end
        end),
    })
end

return user_config
