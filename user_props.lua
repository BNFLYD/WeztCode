-- Preferencias de WeztCode
-- Editá este archivo para personalizar el comportamiento de la app.

user_editor = "nvim"
current_dir = "/home/mori/Projects/Rust/GTK4/WeztCode"

-- Ctrl+J: toggle entre nvim (pane 0) y la última terminal usada (opcional)
-- Descomentar para overridear el default
tab_next = "CTRL+J"

-- Chat IA
llm_provider = "opencode"
llm_model = "deepseek-v4-flash-free"
llm_api_key = "KEYS.OPENCODE"
pi_path = "pi"

-- Backend del chat: "pi" | "little-coder"
-- Si se omite, se auto-detecta (usa little-coder si está instalado).
-- También se puede cambiar en caliente desde el toggle en el header del chat.
-- Los agentes pueden fijar su propio backend con `engine:` en el frontmatter
-- (ver ~/.pi/agent/agents/*.md); eso tiene prioridad sobre este default.
-- agent_backend = "little-coder"

-- Ruta al binario little-coder (opcional, solo si no está en las rutas habituales)
-- lc_path = "/home/mori/.local/share/pnpm/little-coder"

-- STT por mic (voxtype): ruta al binario (opcional, default "voxtype" en PATH)
-- Requiere el daemon corriendo: systemctl --user start voxtype
-- voxtype_path = "voxtype"
