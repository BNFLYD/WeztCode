mod backend;
mod sessions;

pub use backend::*;

use std::thread;

/// Normaliza claves de assets: minúsculas y SOLO alfanuméricos.
/// "stand by" / "stand-by" / "StandBy" → "standby".
fn normalize_asset_key(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Resultado de cambiar de agente: el backend efectivo (post engine-switch)
/// y un warning descriptivo si el engine pedido no pudo aplicarse.
pub struct AgentSwitchOutcome {
    pub backend: BackendFlavor,
    pub warning: Option<String>,
}

/// ChatService con sesiones por scope.
///
/// Cada scope (un sub-agente, o el chat "default" sin agente) tiene SU propia
/// sesión de pi persistida en disco. El selector de agentes es también el
/// selector de conversaciones. El prompt del agente es una copia PERSISTENTE
/// que se re-inyecta solo cuando la sesión del scope está vacía (p. ej. tras
/// "nueva conversación"): si la sesión ya tiene historial, la personalidad ya
/// vive en el contexto y no se duplica.
pub struct ChatService {
    backend: Box<dyn AgentBackend>,
    /// Agente activo: None = chat "default" (sin agente).
    active_agent: Option<String>,
    /// Prompt del agente activo (copia persistente, no de un solo uso).
    active_prompt: Option<String>,
    /// true → el próximo mensaje lleva el prompt prefijado (sesión recién creada).
    prompt_dirty: bool,
    /// session-id en curso (para saber si un switch same-engine necesita cambio).
    current_session_id: Option<String>,
    /// scope → session-id, persistido en disco (sessions.json).
    sessions: sessions::SessionMap,
}

impl ChatService {
    pub fn new(mut backend: Box<dyn AgentBackend>) -> Self {
        if let Err(e) = sync_pi_model_overrides() {
            eprintln!("[chat] Failed to sync pi model overrides: {}", e);
        }

        // Boot scope: el agente default si existe (main.rs ya arma la config con
        // su modelo/engine/thinking), si no el chat "default".
        let default_agent = crate::config::sub_agents::get_default();
        let (scope, prompt, agent_name) = match &default_agent {
            Some(a) => (
                a.name.clone(),
                if a.system_prompt.is_empty() { None } else { Some(a.system_prompt.clone()) },
                Some(a.name.clone()),
            ),
            None => ("default".to_string(), None, None),
        };

        let mut sessions = sessions::SessionMap::load();
        let sid = sessions.get_or_derive(&scope);

        if let Err(e) = backend.spawn(Some(&sid)) {
            eprintln!("[chat] Failed to spawn agent backend: {}", e);
        }

        let mut service = Self {
            backend,
            active_agent: agent_name,
            active_prompt: prompt,
            prompt_dirty: false,
            current_session_id: None,
            sessions,
        };
        let _ = service.refresh_session_state();
        service
    }

    pub fn current_flavor(&self) -> BackendFlavor {
        self.backend.config().flavor
    }

    /// Agente activo (None = chat default). Lo consulta el frontend al bootear.
    pub fn active_agent(&self) -> Option<&str> {
        self.active_agent.as_deref()
    }

    fn active_scope(&self) -> String {
        self.active_agent.clone().unwrap_or_else(|| "default".to_string())
    }

    /// Lee sessionId + messageCount del estado real del proceso. Registra el
    /// session-id real en el mapa del scope activo (p. ej. tras new_session, que
    /// genera un id nuevo) y marca prompt_dirty si la sesión está vacía.
    fn refresh_session_state(&mut self) -> Result<(), String> {
        self.current_session_id = None;
        self.prompt_dirty = false;
        let state = self.backend.get_state().map_err(|e| {
            eprintln!("[chat] refresh_session_state: get_state falló: {}", e);
            format!("El proceso {} no responde tras el cambio: {} (¿murió al bootear? revisá stderr)", self.backend.config().flavor.as_str(), e)
        })?;
        let v = match serde_json::from_str::<serde_json::Value>(&state) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[chat] refresh_session_state: JSON inválido: {}", e);
                return Ok(());
            }
        };
        let data = v.get("data");
        if let Some(sid) = data
            .and_then(|d| d.get("sessionId"))
            .and_then(|s| s.as_str())
        {
            self.current_session_id = Some(sid.to_string());
            let scope = self.active_scope();
            self.sessions.set(&scope, sid.to_string());
        }
        let count = data
            .and_then(|d| d.get("messageCount"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        self.prompt_dirty = count == 0 && self.active_prompt.is_some();
        Ok(())
    }

    /// Asegura que el backend corre con `flavor` y la sesión del scope:
    /// - Cross-engine → respawn con --session-id (retoma/crea la sesión).
    /// - Mismo engine, otra sesión → switch_session RPC en caliente (sin respawn);
    ///   si el archivo no existe (primera vez), respawn la crea.
    fn ensure_session(&mut self, scope: &str, flavor: BackendFlavor) -> Result<(), String> {
        let sid = self.sessions.get_or_derive(scope);

        if self.backend.config().flavor != flavor {
            self.respawn_with_session(flavor, &sid)?;
        } else if self.current_session_id.as_deref() != Some(sid.as_str()) {
            match sessions::find_session_file(&sid) {
                Some(path) => {
                    self.backend
                        .switch_session_rpc(&path.to_string_lossy())?;
                }
                None => {
                    self.respawn_with_session(flavor, &sid)?;
                }
            }
        }
        Ok(())
    }

    /// Respawn del proceso con otro sabor/engine, retomando la sesión indicada.
    /// El prompt lo maneja ChatService (prompt_dirty): nada que preservar acá.
    fn respawn_with_session(&mut self, flavor: BackendFlavor, sid: &str) -> Result<(), String> {
        // Apagar el backend viejo ANTES de spawnear el nuevo. Sin esto, el viejo
        // moría asincrónicamente por EOF de stdin y había una ventana donde dos
        // procesos convivían con el mismo archivo de sesión abierto (little-coder
        // incluso podía huérfanar a su pi nieto) — el proceso nuevo podía morir en
        // esa ventana y el primer mensaje pegaba contra un pipe roto. shutdown()
        // SIGTERM-ea el process group completo y espera el exit real.
        self.backend.shutdown();

        let mut new_config = self.backend.config().clone();
        new_config.flavor = flavor;
        new_config.pi_path = find_pi_path();
        new_config.lc_path = find_little_coder_path();

        let mut new_backend: Box<dyn AgentBackend> = Box::new(PiAgentBackend::new(new_config));
        new_backend.spawn(Some(sid))?;

        self.backend = new_backend;
        // El proceso corre con ESTA sesión (pi la retoma si existe en el dir
        // del proyecto, o la crea con este id si no) — reflejarla para que el
        // próximo ensure_session no force un switch innecesario.
        self.current_session_id = Some(sid.to_string());
        Ok(())
    }

    /// Selector de scope unificado: agente por nombre, o None para el chat default.
    pub fn select_scope(&mut self, agent: Option<&str>) -> Result<AgentSwitchOutcome, String> {
        match agent {
            Some(name) => {
                let entry = crate::config::sub_agents::get_by_name(name)
                    .ok_or_else(|| format!("Agente '{}' no encontrado", name))?;
                self.switch_agent(&entry)
            }
            None => self.switch_default_scope(),
        }
    }

    /// Chat "default": sin agente, sesión propia, modelo default global.
    fn switch_default_scope(&mut self) -> Result<AgentSwitchOutcome, String> {
        self.active_agent = None;
        self.active_prompt = None;
        self.ensure_session("default", default_flavor())?;

        if let Some(m) = crate::config::models::get_default() {
            if self.backend.config().model != m.model {
                self.backend.set_model_rpc(&m.provider, &m.model)?;
            }
        }
        self.refresh_session_state()?;

        Ok(AgentSwitchOutcome {
            backend: self.backend.config().flavor,
            warning: None,
        })
    }

    /// Cambia de agente. La sesión del agente se retoma (o se crea la primera
    /// vez); el prompt del .md se re-inyecta solo si su sesión está vacía.
    pub fn switch_agent(&mut self, entry: &crate::config::sub_agents::SubAgentEntry) -> Result<AgentSwitchOutcome, String> {
        let mut warning: Option<String> = None;

        // Flavor objetivo: engine del agente, o el default global.
        let mut flavor = entry.engine.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(BackendFlavor::parse_strict)
            .unwrap_or_else(default_flavor);

        if matches!(flavor, BackendFlavor::LittleCoder) && find_little_coder_binary().is_none() {
            warning = Some(format!(
                "Agente '{}': engine little-coder no está instalado, se mantiene {}",
                entry.name,
                self.backend.config().flavor.as_str()
            ));
            flavor = self.backend.config().flavor;
        } else if entry.engine.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_some()
            && BackendFlavor::parse_strict(entry.engine.as_deref().unwrap_or("")).is_none()
        {
            warning = Some(format!(
                "Agente '{}': engine '{}' inválido (usá pi o little-coder), se mantiene {}",
                entry.name,
                entry.engine.as_deref().unwrap_or(""),
                self.backend.config().flavor.as_str()
            ));
            flavor = self.backend.config().flavor;
        }

        self.active_agent = Some(entry.name.clone());
        self.active_prompt = if entry.system_prompt.is_empty() {
            None
        } else {
            Some(entry.system_prompt.clone())
        };

        self.ensure_session(&entry.name, flavor)?;

        // Modelo del agente
        let models = crate::config::models::list();
        let agent_model = entry.model.trim();
        if let Some(model_entry) = models.iter().find(|m| m.name.trim().eq_ignore_ascii_case(agent_model)) {
            if self.backend.config().model != model_entry.model {
                self.backend.set_model_rpc(&model_entry.provider, &model_entry.model)?;
            }
        }

        self.refresh_session_state()?;

        Ok(AgentSwitchOutcome {
            backend: self.backend.config().flavor,
            warning,
        })
    }

    /// Cambio manual del engine global (Settings). La sesión y el agente activos
    /// se conservan: el respawn retoma la misma sesión (cambiar de engine nunca
    /// rompe la conversación — pi y little-coder comparten almacenamiento).
    pub fn switch_backend_flavor(&mut self, flavor: BackendFlavor) -> Result<(), String> {
        if self.backend.config().flavor == flavor {
            return Ok(());
        }
        let sid = self
            .current_session_id
            .clone()
            .unwrap_or_else(|| self.sessions.get_or_derive(&self.active_scope()));
        self.respawn_with_session(flavor, &sid)?;
        self.refresh_session_state();
        Ok(())
    }

    /// Cambio manual de modelo desde el dropdown: NO toca el agente ni su
    /// prompt (la personalidad sigue viva en la sesión del scope).
    pub fn switch_model_rpc(&mut self, model_name: &str) -> Result<String, String> {
        let models = crate::config::models::list();
        let entry = models.into_iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| format!("Model '{}' not found", model_name))?;

        let current_model = self.backend.config().model.clone();
        if current_model != entry.model {
            self.backend.set_model_rpc(&entry.provider, &entry.model)?;
        }

        Ok(entry.name)
    }

    /// "Nueva conversación": resetea SOLO la sesión del scope activo. Las demás
    /// conversaciones (otros agentes) quedan intactas. El nuevo session-id se
    /// registra en el mapa para que el próximo respawn retome ESTA y no la vieja.
    pub fn new_conversation(&mut self) -> Result<(), String> {
        self.backend.new_session()?;
        self.refresh_session_state()?;
        Ok(())
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        self.backend.get_session_stats()
    }

    /// Lista las conversaciones del proyecto actual con la sesión actual del
    /// scope activo marcada (para el picker de la UI). JSON serializado.
    pub fn list_conversations(&self) -> Result<String, String> {
        let convs = sessions::list_conversations();
        let current = self.current_session_id.clone();
        let scope = self.active_scope();
        let items: Vec<serde_json::Value> = convs
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "file": c.file,
                    "created": c.created,
                    "modified_ms": c.modified_ms,
                    "size": c.size,
                    "messages": c.messages,
                    "preview": c.preview,
                    "agent": c.agent,
                    "current": current.as_deref() == Some(c.id.as_str()),
                })
            })
            .collect();
        Ok(serde_json::json!({
            "conversations": items,
            "scope": scope,
            "project": crate::config::current_root::get().to_string_lossy(),
        })
        .to_string())
    }

    /// Cambia a una conversación existente del proyecto actual (picker de la
    /// UI). Si la conversación pertenece a OTRO scope (p. ej. se elige una de
    /// Teto estando activa Miku), la persona cambia JUNTO con la conversación
    /// — agente, prompt, modelo y engine — reusando el flujo completo de
    /// select_scope, para no mezclar identidades ni contaminar el mapa
    /// por-proyecto con sesiones ajenas.
    /// Devuelve JSON {agent, scope} con el dueño resultante de la conversación.
    pub fn select_conversation(&mut self, session_id: &str) -> Result<serde_json::Value, String> {
        // Validar que exista en el proyecto actual — sin confiar en paths del
        // cliente.
        let path = sessions::find_session_file(session_id).ok_or_else(|| {
            format!("Conversación '{}' no encontrada en este proyecto", session_id)
        })?;

        // Dueño de la conversación: agente del prompt inyectado, o default.
        let conv_agent = sessions::conversation_agent(&path);

        // Cambio de persona si el dueño difiere del scope activo. select_scope
        // aplica TODO: engine con fallbacks, modelo, prompt y su sesión mapped
        // (que se pisa justo después con la conversación elegida).
        if self.active_agent.as_deref() != conv_agent.as_deref() {
            self.select_scope(conv_agent.as_deref())?;
        }

        // Registrar bajo el scope DUEÑO y retomar exactamente ESTA
        // conversación (switch en caliente al archivo — ensure_session
        // resuelve el path por el id).
        let scope = self.active_scope();
        if self.current_session_id.as_deref() != Some(session_id) {
            self.sessions.set(&scope, session_id.to_string());
            let flavor = self.backend.config().flavor;
            self.ensure_session(&scope, flavor)?;
            self.refresh_session_state()?;
        }

        Ok(serde_json::json!({
            "agent": conv_agent,
            "scope": self.active_scope(),
        }))
    }

    pub fn get_state(&self) -> Result<String, String> {
        self.backend.get_state()
    }

    /// Historial real de la sesión activa (para que la UI refleje la verdad).
    pub fn get_messages(&self) -> Result<String, String> {
        self.backend.get_messages()
    }

    /// Imagen del estado visual de un agente (channel del chat en el monitor).
    /// Resuelve por el agente EXPLÍCITO (parámetro de la UI, no el estado global)
    /// para que el cache del browser key-ee por agent+state y no pegue el asset
    /// de un agente con el URL de otro. `state` se matchea contra el nombre de
    /// archivo ignorando espacios/guiones/bajos y mayúsculas ("standby" matchea
    /// "stand by.gif" y "stand-by.gif" a la vez). Devuelve (content_type, bytes).
    pub fn agent_asset(&self, agent: &str, state: &str) -> Result<(String, Vec<u8>), String> {
        let entry = crate::config::sub_agents::get_by_name(agent)
            .ok_or_else(|| format!("Agente '{}' no encontrado", agent))?;
        let dir_raw = entry.assets.as_deref().map(str::trim).filter(|s| !s.is_empty())
            .ok_or_else(|| format!("Agente '{}' no define assets en su .md", agent))?;

        let dir = std::path::PathBuf::from(dir_raw);
        let want = normalize_asset_key(state);
        if want.is_empty() {
            return Err("Estado vacío".to_string());
        }

        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("assets dir '{}' inaccesible: {}", dir_raw, e))?;
        for fs_entry in entries.flatten() {
            let path = fs_entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if normalize_asset_key(stem) == want {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let content_type = match ext.as_str() {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "webp" => "image/webp",
                    "svg" => "image/svg+xml",
                    _ => "image/gif",
                };
                let bytes = std::fs::read(&path)
                    .map_err(|e| format!("leer {:?}: {}", path, e))?;
                return Ok((content_type.to_string(), bytes));
            }
        }
        Err(format!("Asset '{}' no encontrado en {}", state, dir_raw))
    }

    pub fn restart_backend(&mut self) -> Result<(), String> {
        // Re-resuelve la sesión del scope activo PARA EL PROYECTO ACTUAL (acaba
        // de cambiar). Un spawn(None) arrancaría una sesión nueva en blanco y
        // la conversación real de este proyecto quedaría huérfana hasta el
        // próximo ensure_session. Con el id por-proyecto, el respawn retoma
        // la conversación de ESTE proyecto (o la crea con el id derivado).
        let scope = self.active_scope();
        let flavor = self.backend.config().flavor;
        let sid = self.sessions.get_or_derive(&scope);
        self.respawn_with_session(flavor, &sid)?;
        self.refresh_session_state()
    }

    /// Aborta el turno en curso del backend (pi/little-coder).
    pub fn abort(&mut self) -> Result<(), String> {
        self.backend.abort_stream()
    }

    pub fn send_message_stream(&mut self, message: &str) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        // Inyección del prompt: solo cuando la sesión del scope está vacía
        // (prompt_dirty). Si ya tiene historial, la personalidad ya vive en el
        // contexto — re-inyectar duplicaría las instrucciones.
        let final_message = if self.prompt_dirty {
            self.prompt_dirty = false;
            match &self.active_prompt {
                Some(p) => format!("[System instructions]\n{}\n\n---\n\n{}", p, message),
                None => message.to_string(),
            }
        } else {
            message.to_string()
        };
        // El prompt lo maneja ChatService: el backend no debe inyectar nada extra.
        self.backend.set_agent_prompt(None);

        let mut rx = self.backend.send_message(&final_message)?;
        let (tx, out_rx) = tokio::sync::mpsc::channel::<String>(64);

        thread::spawn(move || {
            while let Some(event) = rx.blocking_recv() {
                let sse_str = event.to_sse_string();
                if tx.blocking_send(sse_str).is_err() { break; }
                if matches!(event, SseEvent::Done) { break; }
            }
        });

        Ok(out_rx)
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        self.backend.shutdown();
    }
}
