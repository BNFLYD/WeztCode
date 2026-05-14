-- WeztCode padding hook - parte del binario, no modificar

local wezterm = require "wezterm"

-- 1. Heredar config del usuario
local user_config_path = os.getenv("WEZTCODE_USER_CONFIG")
local user_config = {}
if user_config_path then
  local ok, result = pcall(dofile, user_config_path)
  if ok then user_config = result or {} end
end

-- 2. Padding cache: solo aplicar override si el valor cambió
local last_padding = nil
local pad_file_path = os.getenv("WEZTCODE_PAD_FILE")

-- 3. update-status: lee archivo de padding y actualiza si cambió
wezterm.on("update-status", function(window, pane)
  if window:window_class() ~= "weztcode-terminal" then return end
  if not pad_file_path then return end

  local f = io.open(pad_file_path, "r")
  if not f then return end
  local content = f:read("*a")
  f:close()

  local value = tonumber(content)
  if value and value ~= last_padding then
    last_padding = value
    local overrides = window:get_config_overrides() or {}
    overrides.window_padding = wezterm.table_merge(
      overrides.window_padding or {},
      { right = value }
    )
    window:set_config_overrides(overrides)
  end
end)

-- 4. Retornar config del usuario
return user_config
