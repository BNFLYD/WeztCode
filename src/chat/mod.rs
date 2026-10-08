mod backend;

pub use backend::*;

use std::thread;

/// Resultado de cambiar de agente: el backend efectivo (post engine-switch)
/// y un warning descriptivo si el engine pedido no pudo aplicarse.
pub struct AgentSwitchOutcome {
    pub backend: BackendFlavor,
    pub warning: Option<String>,
}

pub struct ChatService {
    backend: Box<dyn AgentBackend>,
    default_flavor: BackendFlavor,
}

impl ChatService {
    pub fn new(mut backend: Box<dyn AgentBackend>) -> Self {
        if let Err(e) = sync_pi_model_overrides() {
            eprintln!("[chat] Failed to sync pi model overrides: {}", e);
        }
        let default_flavor = backend.config().flavor;
        if let Err(e) = backend.spawn(None) {
            eprintln!("[chat] Failed to spawn agent backend: {}", e);
        }
        Self { backend, default_flavor }
    }

    pub fn switch_backend(&mut self, new_backend: Box<dyn AgentBackend>) -> Result<(), String> {
        let mut backend = new_backend;
        if let Err(e) = sync_pi_model_overrides() {
            eprintln!("[chat] Failed to sync pi model overrides: {}", e);
        }
        backend.spawn(None)?;
        self.backend = backend;
        Ok(())
    }

    pub fn current_flavor(&self) -> BackendFlavor {
        self.backend.config().flavor
    }

    /// Cambia el backend global default (llamado desde Settings).
    /// Actualiza el default global y, si no hay agente con engine activo, cambia el backend activo.
    pub fn switch_backend_flavor(&mut self, flavor: BackendFlavor) -> Result<(), String> {
        if self.default_flavor == flavor {
            return Ok(());
        }
        self.default_flavor = flavor;

        // Si no hay agente con engine forzando backend, aplicar el cambio
        if self.backend.config().flavor != flavor {
            let current_session_id = self.backend.get_state()
                .ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| v.get("data").and_then(|d| d.get("sessionId")).and_then(|s| s.as_str()).map(str::to_string));

            let mut new_config = self.backend.config().clone();
            new_config.flavor = flavor;
            new_config.pi_path = find_pi_path();
            new_config.lc_path = find_little_coder_path();

            let pending_prompt = self.backend.peek_agent_prompt();

            let mut new_backend: Box<dyn AgentBackend> = Box::new(PiAgentBackend::new(new_config));
            if pending_prompt.is_some() {
                new_backend.set_agent_prompt(pending_prompt);
            }

            new_backend.spawn(current_session_id.as_deref())?;

            self.backend = new_backend;
        }
        Ok(())
    }

    /// Obtiene el flavor default global (para Settings)
    pub fn get_default_flavor(&self) -> BackendFlavor {
        self.default_flavor
    }

    /// Switch backend sin tocar default_flavor (uso interno para agents con engine)
    fn switch_backend_internal(&mut self, flavor: BackendFlavor) -> Result<(), String> {
        if self.backend.config().flavor == flavor {
            return Ok(());
        }

        let current_session_id = self.backend.get_state()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v.get("data").and_then(|d| d.get("sessionId")).and_then(|s| s.as_str()).map(str::to_string));

        let mut new_config = self.backend.config().clone();
        new_config.flavor = flavor;
        new_config.pi_path = find_pi_path();
        new_config.lc_path = find_little_coder_path();

        let pending_prompt = self.backend.peek_agent_prompt();

        let mut new_backend: Box<dyn AgentBackend> = Box::new(PiAgentBackend::new(new_config));
        if pending_prompt.is_some() {
            new_backend.set_agent_prompt(pending_prompt);
        }

        new_backend.spawn(current_session_id.as_deref())?;

        self.backend = new_backend;
        Ok(())
    }

    /// Cambia de agente. Si el agente declara `engine` (pi | little-coder),
    /// primero se respawnea el backend con ese sabor y luego se aplican el
    /// prompt del sistema y el modelo sobre el proceso nuevo (un respawn
    /// descartaría lo aplicado antes). Devuelve el engine efectivo y un
    /// warning opcional (p. ej. engine inválido o little-coder no instalado).
    pub fn switch_agent(&mut self, entry: &crate::config::sub_agents::SubAgentEntry) -> Result<AgentSwitchOutcome, String> {
        let mut warning: Option<String> = None;

        // Determinar flavor objetivo: engine del agente o default global
        let target_flavor = entry.engine.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(BackendFlavor::parse_strict)
            .unwrap_or(self.default_flavor);

        if let Some(engine_raw) = entry.engine.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            match BackendFlavor::parse_strict(engine_raw) {
                Some(BackendFlavor::LittleCoder) if find_little_coder_binary().is_none() => {
                    warning = Some(format!(
                        "Agente '{}': engine little-coder no está instalado, se mantiene {}",
                        entry.name,
                        self.backend.config().flavor.as_str()
                    ));
                }
                Some(flavor) => {
                    if flavor != self.backend.config().flavor {
                        self.switch_backend_internal(flavor)?;
                    }
                }
                None => {
                    warning = Some(format!(
                        "Agente '{}': engine '{}' inválido (usá pi o little-coder), se mantiene {}",
                        entry.name,
                        engine_raw,
                        self.backend.config().flavor.as_str()
                    ));
                }
            }
        } else {
            // Sin engine: usar default global
            if target_flavor != self.backend.config().flavor {
                self.switch_backend_internal(target_flavor)?;
            }
        }

        let prompt = if entry.system_prompt.is_empty() {
            None
        } else {
            Some(entry.system_prompt.clone())
        };
        self.backend.set_agent_prompt(prompt);

        let models = crate::config::models::list();
        let agent_model = entry.model.trim();
        if let Some(model_entry) = models.iter().find(|m| m.name.trim().eq_ignore_ascii_case(agent_model)) {
            let current_model = self.backend.config().model.clone();
            if current_model != model_entry.model {
                self.backend.set_model_rpc(&model_entry.provider, &model_entry.model)?;
            }
        }

        Ok(AgentSwitchOutcome {
            backend: self.backend.config().flavor,
            warning,
        })
    }

    pub fn switch_model_rpc(&mut self, model_name: &str) -> Result<String, String> {
        let models = crate::config::models::list();
        let entry = models.into_iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| format!("Model '{}' not found", model_name))?;

        self.backend.set_agent_prompt(None);

        let current_model = self.backend.config().model.clone();
        if current_model != entry.model {
            self.backend.set_model_rpc(&entry.provider, &entry.model)?;
        }

        Ok(entry.name)
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        self.backend.get_session_stats()
    }

    pub fn get_state(&self) -> Result<String, String> {
        self.backend.get_state()
    }

    pub fn new_session(&mut self) -> Result<(), String> {
        self.backend.new_session()
    }

    pub fn restart_backend(&mut self) -> Result<(), String> {
        self.backend.restart()
    }

    pub fn send_message_stream(&mut self, message: &str) -> Result<tokio::sync::mpsc::Receiver<String>, String> {
        let mut rx = self.backend.send_message(message)?;
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
