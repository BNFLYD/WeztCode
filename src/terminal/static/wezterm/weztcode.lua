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

local FIXED_PADDING = 350
local pad_applied = false

wezterm.on("update-status", function(window, pane)
  if pad_applied then return end

  -- Método más confiable: variable de entorno
  local is_weztcode = os.getenv("WEZTCODE_SESSION") == "true"

  wezterm.log_info("WEZTCODE_SESSION = " .. tostring(os.getenv("WEZTCODE_SESSION")))

  if is_weztcode then
    pad_applied = true
    wezterm.log_info("WeztCode session detected - Applying right padding")

    local base = user_config.window_padding or {}
    local padding = {
      left   = base.left,
      right  = FIXED_PADDING,
      top    = base.top,
      bottom = base.bottom,
    }
    local overrides = window:get_config_overrides() or {}
    overrides.window_padding = padding
    window:set_config_overrides(overrides)
    wezterm.log_info("Padding applied successfully")
  end
end)

return user_config
