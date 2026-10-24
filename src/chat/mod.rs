pub mod backend;
mod sessions;

pub use backend::*;

use std::collections::HashMap;
use std::sync::Arc;
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
    /// UN PROCESO POR SCOPE (agente o "default") — multi-agente paralelo:
    /// cada entrada nace on-demand con el engine/modelo de su .md y queda
    /// viva aunque el usuario switchee: el que trabaja, sigue trabajando.
    backends: HashMap<String, Box<dyn AgentBackend>>,
    /// Estado por scope: session-id en curso + prompt_dirty. Sobrevive
    /// switches de agente (el flag viaja con el scope, no con el activo).
    scope_states: HashMap<String, ScopeState>,
    /// Turno en curso POR scope (buffer + broadcast). Ver `ActiveTurn`.
    active_turns: HashMap<String, std::sync::Arc<ActiveTurn>>,
    /// Agente activo: None = chat "default" (sin agente).
    active_agent: Option<String>,
    /// Prompt del agente activo (copia persistente, no de un solo uso).
    active_prompt: Option<String>,
    /// scope → session-id, persistido en disco (sessions.json).
    sessions: sessions::SessionMap,
}

/// Estado por scope que debe sobrevivir al switch de agente: la sesión en
/// curso de ESE proceso y si su próximo mensaje necesita la persona inyectada.
pub struct ScopeState {
    pub session_id: Option<String>,
    pub prompt_dirty: bool,
    /// Modelo encolado (fase 3): el usuario lo eligió mientras el agente
    /// trabajaba — aplica automáticamente cuando SU turno termina, aunque
    /// esté viendo a otro agente.
    pub pending_model: Option<String>,
    /// Compactación manual en curso para ESTE agente: su RPC tarda decenas
    /// de segundos (resume del LLM) — los demás endpoints de este scope
    /// fast-failan mientras tanto (mismo esquema que un turno activo).
    pub compacting: bool,
}

/// Turno de chat en curso: eventos acumulados + canal live para re-attach.
///
/// El forwarder SIEMPRE drena pi aunque no haya cliente SSE conectado.
/// Antes, si el cliente se desconectaba (cambio de sección), el forwarder
/// cortaba en el send fallido → el canal del reader llenaba su capacidad →
/// nadie drenaba el stdout de pi → el pipe del SO (64KB) se llenaba → pi
/// quedaba COLGADO escribiendo (el agente se congelaba de verdad). Con el
/// buffer, el turno sobrevive al ausencia del cliente y cualquier UI puede
/// re-attacharse después (GET /chat/stream): replay del buffer + live.
pub struct ActiveTurn {
    /// Agente dueño del turno (para que la UI sepa quién está trabajando).
    pub agent: Option<String>,
    events: std::sync::Mutex<Vec<String>>,
    live: tokio::sync::broadcast::Sender<String>,
    done: std::sync::atomic::AtomicBool,
}

impl ActiveTurn {
    /// Suscribirse a los eventos live Y capturar el buffer actual, sin
    /// huecos ni duplicados: la suscripción y el snapshot van bajo el mismo
    /// lock con el que el forwarder publica (snapshot-then-live exacto).
    pub fn attach(&self) -> (Vec<String>, tokio::sync::broadcast::Receiver<String>) {
        let events = self.events.lock().unwrap();
        let snapshot = events.clone();
        let rx = self.live.subscribe();
        (snapshot, rx)
    }

    pub fn is_done(&self) -> bool {
        self.done.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn publish(&self, sse: String) {
        let mut events = self.events.lock().unwrap();
        events.push(sse.clone());
        // broadcast::send no bloquea; sin subscriptores es Err (no importa:
        // el buffer es la fuente de verdad para el replay del re-attach).
        let _ = self.live.send(sse);
    }

    fn finish(&self) {
        self.done.store(true, std::sync::atomic::Ordering::SeqCst);
    }
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

        let prompt_dirty = prompt.is_some() && sessions::session_is_empty(&sid);
        let mut service = Self {
            backends: HashMap::new(),
            scope_states: HashMap::new(),
            active_turns: HashMap::new(),
            active_agent: agent_name,
            active_prompt: prompt,
            sessions,
        };
        // El backend de arranque (main ya lo spawn-eó con el scope default)
        // entra al mapa bajo su scope. SIN refresh_session_state: su get_state
        // espera el boot completo de pi CON EL LOCK DEL SERVICIO TOMADO —
        // inspección de archivo en su lugar (pi respeta ids custom).
        let boot_scope = service.active_scope();
        service.backends.insert(boot_scope.clone(), backend);
        service.scope_states.insert(
            boot_scope,
            ScopeState {
                session_id: Some(sid),
                prompt_dirty,
                pending_model: None,
                compacting: false,
            },
        );
        service
    }

    pub fn current_flavor(&self) -> BackendFlavor {
        self.active_backend().config().flavor
    }

    // ═══════════ Accesores per-scope (mapas) ═══════════

    fn active_backend(&self) -> &Box<dyn AgentBackend> {
        self.backends
            .get(&self.active_scope())
            .expect("backend del scope activo siempre existe")
    }

    fn active_backend_mut(&mut self) -> &mut Box<dyn AgentBackend> {
        let scope = self.active_scope();
        self.backends
            .get_mut(&scope)
            .expect("backend del scope activo siempre existe")
    }

    /// session-id en curso del scope ACTIVO.
    fn active_session_id(&self) -> Option<&str> {
        self.scope_states
            .get(&self.active_scope())
            .and_then(|st| st.session_id.as_deref())
    }

    /// ¿El scope tiene un turno corriendo? (para guards de RPC starvation)
    fn turn_running(&self, scope: &str) -> bool {
        self.active_turns
            .get(scope)
            .map(|t| !t.is_done())
            .unwrap_or(false)
    }

    /// Asegura que el scope tenga su proceso: spawn on-demand con el
    /// engine/modelo resueltos de su .md y la sesión mapeada. Los backends
    /// existentes NO se tocan — el que trabaja, sigue trabajando.
    fn ensure_backend(&mut self, scope: &str, mut config: ChatConfig, sid: &str) -> Result<(), String> {
        if self.backends.contains_key(scope) {
            return Ok(());
        }
        config.pi_path = find_pi_path();
        config.lc_path = find_little_coder_path();
        let mut b: Box<dyn AgentBackend> = Box::new(PiAgentBackend::new(config));
        b.spawn(Some(sid))?;
        let dirty = sessions::session_is_empty(sid)
            && crate::config::sub_agents::get_by_name(scope)
                .map(|e| !e.system_prompt.is_empty())
                .unwrap_or(false);
        self.backends.insert(scope.to_string(), b);
        self.scope_states.insert(
            scope.to_string(),
            ScopeState { session_id: Some(sid.to_string()), prompt_dirty: dirty, pending_model: None, compacting: false },
        );
        Ok(())
    }

    /// Respawnea el proceso de UN scope con un flavor/sesión dados (mata solo
    /// ese proceso — los demás agentes no se enteran).
    fn respawn_scope(&mut self, scope: &str, flavor: BackendFlavor, sid: &str) -> Result<(), String> {
        let mut old = self.backends.remove(scope)
            .ok_or_else(|| format!("Scope '{}' sin backend", scope))?;
        old.shutdown(); // SIGTERM al process group completo + espera el exit real

        let mut new_config = old.config().clone();
        new_config.flavor = flavor;
        new_config.pi_path = find_pi_path();
        new_config.lc_path = find_little_coder_path();

        let mut new_backend: Box<dyn AgentBackend> = Box::new(PiAgentBackend::new(new_config));
        new_backend.spawn(Some(sid))?;

        if let Some(st) = self.scope_states.get_mut(scope) {
            st.session_id = Some(sid.to_string());
        }
        self.backends.insert(scope.to_string(), new_backend);
        Ok(())
    }

    /// Cambia la sesión EN CALIENTE del scope ACTIVO (picker: retomar otra
    /// conversación del mismo agente). Primera vez sin archivo → respawn.
    fn switch_active_session(&mut self, sid: &str) -> Result<(), String> {
        if self.active_session_id() == Some(sid) {
            return Ok(());
        }
        let scope = self.active_scope();
        let path = sessions::find_session_file(sid);
        match path {
            Some(p) => {
                self.active_backend_mut()
                    .switch_session_rpc(&p.to_string_lossy())?;
            }
            None => {
                let flavor = self.active_backend().config().flavor;
                self.respawn_scope(&scope, flavor, sid)?;
            }
        }
        let scope = self.active_scope();
        if let Some(st) = self.scope_states.get_mut(&scope) {
            st.session_id = Some(sid.to_string());
        }
        Ok(())
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
        let scope = self.active_scope();
        if let Some(st) = self.scope_states.get_mut(&scope) {
            st.session_id = None;
            st.prompt_dirty = false;
        }
        let state = self.active_backend().get_state().map_err(|e| {
            eprintln!("[chat] refresh_session_state: get_state falló: {}", e);
            format!("El proceso {} no responde tras el cambio: {} (¿murió al bootear? revisá stderr)", self.active_backend().config().flavor.as_str(), e)
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
            let scope = self.active_scope();
            self.sessions.set(&scope, sid.to_string());
            if let Some(st) = self.scope_states.get_mut(&scope) {
                st.session_id = Some(sid.to_string());
            }
        }
        let count = data
            .and_then(|d| d.get("messageCount"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        if let Some(st) = self.scope_states.get_mut(&self.active_scope()) {
            st.prompt_dirty = count == 0 && self.active_prompt.is_some();
        }
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

        let sid = self.sessions.get_or_derive("default");
        self.ensure_backend("default", ChatConfig::from_default_model(), &sid)?;

        // Modelo default global: el proceso nace con él; re-sincronizar solo
        // si ya existía y está idle (set_model_rpc con turno activo se muere
        // de hambre esperando el stdout del stream).
        if let Some(m) = crate::config::models::get_default() {
            if self.active_backend().config().model != m.model
                && !self.turn_running("default")
            {
                self.active_backend_mut().set_model_rpc(&m.provider, &m.model)?;
            }
        }

        Ok(AgentSwitchOutcome {
            backend: self.active_backend().config().flavor,
            warning: None,
        })
    }

    /// Cambia de agente. El switch SOLO mueve el puntero activo: el proceso
    /// del agente nace on-demand con su engine/modelo/sesión, y el de otro
    /// agente que esté trabajando sigue vivo — nunca se pisan entre sí.
    pub fn switch_agent(&mut self, entry: &crate::config::sub_agents::SubAgentEntry) -> Result<AgentSwitchOutcome, String> {
        let mut warning: Option<String> = None;

        // Flavor objetivo: engine del agente con los fallbacks globales.
        let mut flavor = entry.engine.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .and_then(BackendFlavor::parse_strict)
            .unwrap_or_else(default_flavor);
        if matches!(flavor, BackendFlavor::LittleCoder) && find_little_coder_binary().is_none() {
            warning = Some(format!(
                "Agente '{}': engine little-coder no está instalado, se usa pi",
                entry.name
            ));
            flavor = BackendFlavor::Pi;
        } else if entry.engine.as_deref().map(str::trim).filter(|s| !s.is_empty()).is_some()
            && BackendFlavor::parse_strict(entry.engine.as_deref().unwrap_or("")).is_none()
        {
            warning = Some(format!(
                "Agente '{}': engine '{}' inválido (usá pi o little-coder), se usa el default",
                entry.name,
                entry.engine.as_deref().unwrap_or("")
            ));
            flavor = default_flavor();
        }

        // Persona activa (el prompt_dirty viaja en el ScopeState del agente,
        // no acá: sobrevive ida y vuelta entre agentes).
        self.active_agent = Some(entry.name.clone());
        self.active_prompt = if entry.system_prompt.is_empty() {
            None
        } else {
            Some(entry.system_prompt.clone())
        };

        // Proceso del agente: spawn on-demand con SU flavor y SU sesión.
        let sid = self.sessions.get_or_derive(&entry.name);
        let mut config = ChatConfig::from_sub_agent(entry);
        config.flavor = flavor;
        self.ensure_backend(&entry.name, config, &sid)?;

        // Modelo del agente: el proceso nace con él; re-sincronizar solo si
        // ya existía y está idle (un agente trabajando no se toca).
        let models = crate::config::models::list();
        if let Some(model_entry) =
            models.iter().find(|m| m.name.trim().eq_ignore_ascii_case(entry.model.trim()))
        {
            if self.active_backend().config().model != model_entry.model
                && !self.turn_running(&entry.name)
            {
                self.active_backend_mut().set_model_rpc(&model_entry.provider, &model_entry.model)?;
            }
        }

        Ok(AgentSwitchOutcome {
            backend: self.active_backend().config().flavor,
            warning,
        })
    }

    /// Cambio manual del engine (Settings): respawnea SOLO el proceso del
    /// scope ACTIVO con el mismo session-id (cambiar de engine nunca rompe
    /// la conversación — pi y little-coder comparten almacenamiento). Los
    /// demás agentes no se tocan.
    pub fn switch_backend_flavor(&mut self, flavor: BackendFlavor) -> Result<(), String> {
        if self.active_backend().config().flavor == flavor {
            return Ok(());
        }
        let scope = self.active_scope();
        let sid = self
            .active_session_id()
            .map(str::to_string)
            .unwrap_or_else(|| self.sessions.get_or_derive(&scope));
        self.respawn_scope(&scope, flavor, &sid)?;
        self.refresh_session_state()?;
        Ok(())
    }

    /// Cambio de modelo desde el dropdown = CONFIGURACIÓN del agente (fase 3):
    /// - Agente trabajando → se ENCOLA en su ScopeState: un set_model_rpc con
    ///   el reader del stream consumiendo el stdout se muere de hambre (el
    ///   pipe del agente queda intacto). Aplica cuando SU turno termina,
    ///   esté quien sea viendo la UI.
    /// - Idle → aplica ya al proceso + persiste al .md del agente: nace con
    ///   él en respawns/switches/boots, y el switch_agent nunca lo revierte.
    /// Devuelve JSON {model, queued, persisted}.
    pub fn switch_model_rpc(&mut self, model_name: &str) -> Result<String, String> {
        let models = crate::config::models::list();
        let entry = models.into_iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| format!("Model '{}' not found", model_name))?;

        let scope = self.active_scope();
        if self.turn_running(&scope) {
            if let Some(st) = self.scope_states.get_mut(&scope) {
                st.pending_model = Some(entry.name.clone());
            }
            return Ok(serde_json::json!({
                "model": entry.name,
                "queued": true,
                "persisted": false,
            })
            .to_string());
        }
        let persisted = self.apply_model_to_agent(&scope, &entry)?;
        Ok(serde_json::json!({
            "model": entry.name,
            "queued": false,
            "persisted": persisted,
        })
        .to_string())
    }

    /// Aplica el modelo al proceso del scope (idle) y persiste al .md si el
    /// scope es un agente (el chat default no tiene .md: su modelo es el
    /// global, efímero por diseño). Devuelve si persistió.
    fn apply_model_to_agent(
        &mut self,
        scope: &str,
        entry: &crate::config::models::ModelEntry,
    ) -> Result<bool, String> {
        let backend = self
            .backends
            .get_mut(scope)
            .ok_or_else(|| format!("Scope '{}' sin backend", scope))?;
        if backend.config().model != entry.model {
            backend.set_model_rpc(&entry.provider, &entry.model)?;
        }
        if scope != "default" {
            crate::config::sub_agents::update_agent_model(scope, &entry.name)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Aplica el modelo encolado del scope — lo llama el forwarder cuando el
    /// turno de ESE scope termina (el backend es dueño de la cola: aplica
    /// aunque la UI esté viendo a otro agente).
    pub fn apply_pending_model(&mut self, scope: &str) {
        let pending = match self.scope_states.get_mut(scope) {
            Some(st) => st.pending_model.take(),
            None => return,
        };
        let Some(model_name) = pending else { return };
        if self.turn_running(scope) {
            // Otro turno arrancó justo antes: re-encolar para el próximo fin
            if let Some(st) = self.scope_states.get_mut(scope) {
                st.pending_model = Some(model_name);
            }
            return;
        }
        let models = crate::config::models::list();
        if let Some(entry) = models.into_iter().find(|m| m.name == model_name) {
            if let Err(e) = self.apply_model_to_agent(scope, &entry) {
                eprintln!("[chat] modelo pendiente de '{}': {}", scope, e);
            }
        }
    }

    /// "Nueva conversación": resetea SOLO la sesión del scope activo. Las demás
    /// conversaciones (otros agentes) quedan intactas. El nuevo session-id se
    /// registra en el mapa para que el próximo respawn retome ESTA y no la vieja.
    pub fn new_conversation(&mut self) -> Result<(), String> {
        if self.turn_running(&self.active_scope()) {
            return Err("Turno en curso: esperá a que termine".to_string());
        }
        self.active_backend_mut().new_session()?;
        self.refresh_session_state()?; // pi inventó un sid nuevo: leerlo real
        Ok(())
    }

    pub fn get_session_stats(&self) -> Result<String, String> {
        // Fast-fail durante turno activo del scope visto — misma razón que
        // get_messages (el stdout de pi lo consume el reader del stream).
        if self.turn_running(&self.active_scope()) {
            return Err("Turno en curso".to_string());
        }
        if self.scope_compacting(&self.active_scope()) {
            return Err("Compactando el contexto".to_string());
        }
        self.active_backend().get_session_stats()
    }

    /// Elimina el archivo de una conversación del proyecto actual. La
    /// conversación ACTIVA también se puede borrar por pedido explícito: se
    /// arranca una conversación nueva en el mismo scope y se elimina el
    /// archivo de la vieja (pi ya no la usa tras el new_session). El path
    /// sale de find_session_file, que solo lista el directorio de sesiones
    /// del proyecto actual — no puede escapar de ahí.
    pub fn delete_conversation(&mut self, session_id: &str) -> Result<(), String> {
        if self.active_session_id() == Some(session_id) {
            // Sin turno en curso: un new_session mientras el reader consume
            // el stdout de pi se muere de hambre (mismo lock que get_messages).
            if self.turn_running(&self.active_scope()) {
                return Err(
                    "El agente está trabajando: esperá a que termine el turno".to_string(),
                );
            }
            self.active_backend_mut().new_session()?;
            self.refresh_session_state()?; // registra el sid NUEVO en el mapa
        }
        let path = sessions::find_session_file(session_id).ok_or_else(|| {
            format!("Conversación '{}' no encontrada en este proyecto", session_id)
        })?;
        std::fs::remove_file(&path)
            .map_err(|e| format!("No se pudo eliminar: {}", e))
    }

    /// Lista las conversaciones del proyecto actual con la sesión actual del
    /// scope activo marcada (para el picker de la UI). JSON serializado.
    pub fn list_conversations(&self) -> Result<String, String> {
        let convs = sessions::list_conversations();
        let current = self.active_session_id().map(str::to_string);
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
        // conversación (switch en caliente al archivo del proceso de ESE
        // agente — el resto no se toca).
        let scope = self.active_scope();
        if self.active_session_id() != Some(session_id) {
            self.sessions.set(&scope, session_id.to_string());
            self.switch_active_session(session_id)?;
            // prompt_dirty de la conversación elegida (por archivo — sin
            // get_state que espere el boot del proceso).
            let dirty = sessions::session_is_empty(session_id)
                && self.active_prompt.is_some();
            if let Some(st) = self.scope_states.get_mut(&scope) {
                st.prompt_dirty = dirty;
            }
        }

        Ok(serde_json::json!({
            "agent": conv_agent,
            "scope": self.active_scope(),
        }))
    }

    pub fn get_state(&self) -> Result<String, String> {
        self.active_backend().get_state()
    }

    /// Historial real de la sesión activa (para que la UI refleje la verdad).
    pub fn get_messages(&self) -> Result<String, String> {
        // Durante un turno activo, el reader del stream sostiene el stdout
        // de pi hasta el agent_end: un RPC concurrente se muere de hambre
        // esperando ese lock — y este handler sostiene el lock del SERVICIO
        // mientras espera, congelando todos los demás endpoints (active-agent,
        // sub-agents...). Con turno en curso, el stream ES la verdad; el
        // historial se pide recién al terminar.
        if self.turn_running(&self.active_scope()) {
            return Err("Turno en curso: el historial se actualiza al terminar".to_string());
        }
        if self.scope_compacting(&self.active_scope()) {
            return Err("Compactando el contexto: probá en unos segundos".to_string());
        }
        self.active_backend().get_messages()
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
        // Cambio de proyecto: TODOS los procesos mueren (su cwd cambió), los
        // estados por scope se resetean y el activo renace on-demand con la
        // sesión mapeada de ESTE proyecto (por-proyecto en el mapa). Los
        // demás agentes nacen cuando el usuario los llame.
        let scope = self.active_scope();
        let active_config = self.active_backend().config().clone();
        let flavor = active_config.flavor;
        for (_, mut b) in std::mem::take(&mut self.backends) {
            b.shutdown();
        }
        self.scope_states.clear();
        self.active_turns.clear();

        let sid = self.sessions.get_or_derive(&scope);
        self.ensure_backend(&scope, active_config, &sid)?;
        // ensure_backend NO aplicará el flavor correcto si cambió... ya lo
        // lleva en active_config ✓. prompt_dirty sale del archivo.
        let _ = flavor;
        Ok(())
    }

    /// /compact manual del scope activo: resume el contexto de SU sesión
    /// (el mismo /compact del CLI de pi). El RPC tarda decenas de segundos —
    /// por eso va en DOS fases: compact_begin toma el lock SOLO para
    /// validar y extraer los handles del proceso, y el RPC corre FUERA del
    /// lock (compact RPC en handle_chat_compact). Sin esto, el handler
    /// sostenía el lock del servicio durante TODO el resume y congelaba a
    /// todos los demás endpoints (switch de agente, active-agent, init).
    pub fn compact_begin(
        &mut self,
    ) -> Result<
        (
            String,
            std::sync::Arc<std::sync::Mutex<std::process::ChildStdin>>,
            std::sync::Arc<std::sync::Mutex<std::process::ChildStdout>>,
        ),
        String,
    > {
        let scope = self.active_scope();
        if self.turn_running(&scope) {
            return Err("El agente está trabajando: compactá al terminar el turno".to_string());
        }
        let compacting = self
            .scope_states
            .get(&scope)
            .map(|st| st.compacting)
            .unwrap_or(false);
        if compacting {
            return Err("Ya se está compactando el contexto".to_string());
        }
        let handles = self
            .active_backend()
            .compact_handles()
            .ok_or_else(|| "compact not supported by this backend".to_string())?;
        if let Some(st) = self.scope_states.get_mut(&scope) {
            st.compacting = true;
        }
        Ok((scope, handles.0, handles.1))
    }

    /// Fase final del compact (con lock, breve): libera el flag del scope
    /// ORIGINAL — el usuario puede haber cambiado de agente durante el
    /// resume (decenas de segundos con el servicio libre, por diseño).
    pub fn compact_end(&mut self, scope: &str) {
        if let Some(st) = self.scope_states.get_mut(scope) {
            st.compacting = false;
        }
    }

    /// ¿El scope está compactando? (guards de fast-fail de RPCs)
    fn scope_compacting(&self, scope: &str) -> bool {
        self.scope_states
            .get(scope)
            .map(|st| st.compacting)
            .unwrap_or(false)
    }

    /// Aborta el turno en curso del backend (pi/little-coder).
    pub fn abort(&mut self) -> Result<(), String> {
        self.active_backend_mut().abort_stream()
    }

    pub fn send_message_stream(
        &mut self,
        message: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<String>, String> {
        // Guard: un turno a la vez POR AGENTE — el turno de OTRO scope no
        // impide enviarle a este (multi-agente paralelo).
        let scope = self.active_scope();
        if self.turn_running(&scope) {
            return Err("Ya hay un turno en curso en este agente".to_string());
        }
        if self.scope_compacting(&scope) {
            return Err("Compactando el contexto: esperá a que termine".to_string());
        }

        // Inyección del prompt: solo cuando la sesión del scope está vacía
        // (prompt_dirty, viaja en el ScopeState del agente). Si ya tiene
        // historial, la personalidad ya vive en el contexto.
        let dirty = self.scope_states.get(&scope).map(|st| st.prompt_dirty).unwrap_or(false);
        let final_message = if dirty {
            if let Some(st) = self.scope_states.get_mut(&scope) {
                st.prompt_dirty = false;
            }
            match &self.active_prompt {
                Some(p) => format!("[System instructions]\n{}\n\n---\n\n{}", p, message),
                None => message.to_string(),
            }
        } else {
            message.to_string()
        };
        // El prompt lo maneja ChatService: el backend no debe inyectar nada extra.
        self.active_backend_mut().set_agent_prompt(None);

        let mut rx = self.active_backend_mut().send_message(&final_message)?;

        // Turno activo: buffer + broadcast. La suscripción del que envía se
        // crea ANTES de spawnear el forwarder → no se pierde ningún evento.
        let (live_tx, out_rx) = tokio::sync::broadcast::channel::<String>(1024);
        let turn = Arc::new(ActiveTurn {
            agent: self.active_agent.clone(),
            events: std::sync::Mutex::new(Vec::new()),
            live: live_tx,
            done: std::sync::atomic::AtomicBool::new(false),
        });
        self.active_turns.insert(scope.clone(), turn.clone());

        thread::spawn(move || {
            // Drena SIEMPRE hasta el final (Done o EOF) aunque no haya nadie
            // escuchando: pi sigue vivo aunque la UI se haya ido a otra
            // sección — el turno queda en el buffer para el re-attach.
            while let Some(event) = rx.blocking_recv() {
                let sse_str = event.to_sse_string();
                let is_done = matches!(event, SseEvent::Done);
                turn.publish(sse_str);
                if is_done {
                    break;
                }
            }
            turn.finish();
            // Fase 3: el turno terminó — aplicar el modelo que el usuario
            // encoló para ESTE agente mientras trabajaba (aunque la UI esté
            // viendo a otro). El lock del servicio está libre acá: nadie lo
            // sostiene esperando el fin del turno (los RPCs fast-failan).
            if let Ok(mut service) = crate::CHAT_SERVICE.lock() {
                service.apply_pending_model(&scope);
            }
        });

        Ok(out_rx)
    }

    /// Estado del turno de UN scope para el re-attach de la UI
    /// (GET /chat/stream?agent=X; None = el activo). None si ese scope no
    /// tiene turno en curso: la UI reconcilia con el historial.
    pub fn attach_turn(
        &self,
        scope: Option<&str>,
    ) -> Result<
        Option<(
            Option<String>,
            Vec<String>,
            tokio::sync::broadcast::Receiver<String>,
        )>,
        String,
    > {
        let scope = match scope {
            Some(s) => s.to_string(),
            None => self.active_scope(),
        };
        match self.active_turns.get(&scope) {
            Some(t) if !t.is_done() => {
                let (snapshot, live_rx) = t.attach();
                Ok(Some((t.agent.clone(), snapshot, live_rx)))
            }
            _ => Ok(None),
        }
    }
}

impl Drop for ChatService {
    fn drop(&mut self) {
        for (_, mut b) in std::mem::take(&mut self.backends) {
            b.shutdown();
        }
    }
}
