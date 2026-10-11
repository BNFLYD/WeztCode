mod backend;
mod sessions;

pub use backend::*;

use std::thread;

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
        service.refresh_session_state();
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
    fn refresh_session_state(&mut self) {
        self.current_session_id = None;
        self.prompt_dirty = false;
        let Ok(state) = self.backend.get_state() else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&state) else { return };
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
        let mut new_config = self.backend.config().clone();
        new_config.flavor = flavor;
        new_config.pi_path = find_pi_path();
        new_config.lc_path = find_little_coder_path();

        let mut new_backend: Box<dyn AgentBackend> = Box::new(PiAgentBackend::new(new_config));
        new_backend.spawn(Some(sid))?;

        self.backend = new_backend;
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
        self.refresh_session_state();

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

        self.refresh_session_state();

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
        self.refresh_session_state();
        Ok(())
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        self.backend.get_session_stats()
    }

    pub fn get_state(&self) -> Result<String, String> {
        self.backend.get_state()
    }

    /// Historial real de la sesión activa (para que la UI refleje la verdad).
    pub fn get_messages(&self) -> Result<String, String> {
        self.backend.get_messages()
    }

    pub fn restart_backend(&mut self) -> Result<(), String> {
        self.backend.restart()
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
