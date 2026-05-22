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

-- 2. Agregar domain weztcode para CLI (siempre, incluso si el usuario tiene domains)
local weztcode_domain = { name = "weztcode", socket_path = "/tmp/weztcode-wezterm.sock" }
if user_config.unix_domains == nil then
  user_config.unix_domains = { weztcode_domain }
else
  table.insert(user_config.unix_domains, weztcode_domain)
end

-- 3. Leer side_pad para padding dinámico
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

-- 4. Aplicar padding si hay valor válido (se ejecuta en reload-config)
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

-- 5. update-status: reintentar si aún no se pudo aplicar
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

return user_config
