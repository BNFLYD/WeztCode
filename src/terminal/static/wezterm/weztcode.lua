-- WeztCode padding hook - parte del binario, no modificar

local wezterm = require "wezterm"

wezterm.log_info("weztcode.lua loaded")

-- 1. Heredar config del usuario
local user_config_path = os.getenv("WEZTCODE_USER_CONFIG")
wezterm.log_info("WEZTCODE_USER_CONFIG = " .. tostring(user_config_path))

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

-- 2. Padding fijo de 350px para debug
local FIXED_PADDING = 350
local pad_applied = false

-- 3. update-status: aplica padding fijo una vez
wezterm.on("update-status", function(window, pane)
  wezterm.log_info("update-status fired")

  local wc = window:window_class()
  wezterm.log_info("window_class = " .. tostring(wc))

  if wc ~= "weztcode-terminal" then return end

  if not pad_applied then
    pad_applied = true
    wezterm.log_info("Applying fixed padding: " .. FIXED_PADDING)
    local overrides = window:get_config_overrides() or {}
    overrides.window_padding = wezterm.table_merge(
      overrides.window_padding or {},
      { right = FIXED_PADDING }
    )
    window:set_config_overrides(overrides)
    wezterm.log_info("Fixed padding applied")
  end
end)

-- 4. Retornar config del usuario
wezterm.log_info("Returning user config")
return user_config
