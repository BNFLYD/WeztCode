-- WeztCode padding hook
local wezterm = require "wezterm"
wezterm.log_info("weztcode.lua loaded")

local user_config_path = os.getenv("WEZTCODE_USER_CONFIG")
local user_config = {}

if user_config_path then
  local ok, result = pcall(dofile, user_config_path)
  if ok then
    user_config = result or {}
    wezterm.log_info("User config loaded successfully")
  end
end

local FIXED_PADDING = 350
local pad_applied = false

wezterm.on("update-status", function(window, pane)
  if pad_applied then return end

  local title = window:get_title() or ""
  local class = window:window_class() or ""

  wezterm.log_info("Window title: " .. title)
  wezterm.log_info("Window class: " .. class)

  if title:find("weztcode") or class:find("weztcode") then
    pad_applied = true
    wezterm.log_info("WezTerm WeztCode detected - Applying padding")

    local overrides = window:get_config_overrides() or {}
    overrides.window_padding = wezterm.table_merge(
      overrides.window_padding or {},
      { right = FIXED_PADDING }
    )
    window:set_config_overrides(overrides)
  end
end)

return user_config
