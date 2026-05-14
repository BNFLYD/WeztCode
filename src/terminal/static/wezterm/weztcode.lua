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

-- 2. Usar evento más confiable
wezterm.on("update-status", function(window, pane)
  if pad_applied then return end

  -- Método más confiable: usar el título de la pestaña/pane
  local tab = window:active_tab()
  local tab_title = tab:get_title() or ""

  wezterm.log_info("Tab title = " .. tab_title)

  -- Buscamos por una marca que ponemos al lanzar Wezterm
  if tab_title:find("weztcode") or tab_title:find("WeztCode") then
    pad_applied = true
    wezterm.log_info("WeztCode terminal detected - Applying right padding")

    local overrides = window:get_config_overrides() or {}
    overrides.window_padding = wezterm.table_merge(
      overrides.window_padding or {},
      { right = FIXED_PADDING }
    )
    window:set_config_overrides(overrides)
  end
end)

return user_config
