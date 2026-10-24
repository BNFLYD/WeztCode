<script>
  import { onMount, tick } from "svelte";
  import Icon from "@iconify/svelte";
  import ChatChannel from "../ui/channels/chat_channel.svelte";
  // Reuso del diálogo de confirmación Si/No del canal del explorer (mismo
  // cartel, mismo teclado) para eliminar conversaciones.
  import ExplorerChannel from "../ui/channels/explorer_channel.svelte";

  // Bindings del monitor (runes: bindable): el channel del chat dibuja el
  // visual del agente activo según su estado (assets de su .md).
  let {
    active_section = "chat",
    active_channel = $bindable(null),
    is_distorting = $bindable(false),
  } = $props();

  const STORAGE_KEY = "weztcode_chat_messages";

  // Scope activo: "default" o el nombre del agente. Cada scope tiene su propia
  // conversación (sesión persistida por agente en el backend).
  let active_scope = $state("default");

  // Estado visual del agente para el channel del monitor:
  // standby | talking | thinking | confused (según el streaming de pi/lc).
  let agent_visual = $state("standby");
  // Ownership del channel: solo lo soltamos si lo pusimos nosotros. Evita pisar
  // el channel del explorer cuando el modelo streamea desde otra sección.
  let channel_owned = $state(false);

  function set_visual(state) {
    if (agent_visual !== state) agent_visual = state;
  }

  function flash_static(ms = 400) {
    is_distorting = true;
    setTimeout(() => (is_distorting = false), ms);
  }

  // Channel del monitor: visible SOLO en la sección chat y SOLO si el agente
  // activo define assets. Nunca lee active_channel (evita loops de $effect).
  // Fuera de la sección chat los cambios de estado visual son no-op: el canal
  // de otra sección queda intacto.
  $effect(() => {
    const in_chat = active_section === "chat";

    // El diálogo de confirmación (borrar conversación) vive en el MONITOR:
    // mientras esté abierto, NO pisarlo con el visual del agente. Si el
    // usuario se fue de la sección con el diálogo abierto, +page nuló el
    // canal: cancelar la confirmación pendiente para no dejar estado huérfano.
    if (conv_delete_confirm) {
      if (!in_chat) conv_delete_confirm = null;
      return;
    }

    const agent = current_agent
      ? sub_agents.find(a => a.name === current_agent)
      : null;
    const show = in_chat && !!agent?.assets;

    if (show) {
      active_channel = {
        component: ChatChannel,
        props: { state: agent_visual, agent_name: current_agent },
      };
      channel_owned = true;
    } else if (channel_owned) {
      active_channel = null;
      channel_owned = false;
    }
  });

  // Proyecto actual (viene del backend en /chat/active-agent). Key-ea el
  // cache de mensajes por proyecto+agente: el instant-paint tras un switch
  // muestra el cache de ESTE proyecto, nunca la conversación de otro.
  let current_project = $state("");

  function scope_key(scope) {
    const base = scope === "default" ? STORAGE_KEY : `${STORAGE_KEY}__${scope}`;
    return current_project ? `${base}__${current_project}` : base;
  }
  function save_scope(scope) {
    const raw = JSON.stringify(messages);
    // localStorage tiene ~5MB, pero prevenimos truncamiento
    try {
      localStorage.setItem(scope_key(scope), raw);
    } catch {
      // Si excede cuota, guardamos solo los últimos 50 mensajes
      const trimmed = messages.slice(-50);
      localStorage.setItem(scope_key(scope), JSON.stringify(trimmed));
    }
  }
  function load_scope(scope) {
    return JSON.parse(localStorage.getItem(scope_key(scope)) || "[]");
  }

  let messages = $state(load_scope("default"));
  let input_value = $state("");
  let streaming = $state(false);
  let list_ref = $state(null);
  let warnings = $state([]);
  let show_warnings = $state(false);
  let models = $state([]);
  let current_model = $state("");
  let switching = $state(false);
  let show_dropdown = $state(false);
  let dropdown_container = $state(null);
  let real_context_percent = $state(null);
  let real_context_window = $state(null);
  let sub_agents = $state([]);
  let builtins = $state([]);
  let current_agent = $state(null);
  let current_icon = $state(null);
  let show_agent_dropdown = $state(false);
  let agent_dropdown_container = $state(null);

  // Picker de conversaciones del proyecto actual (botón de búsqueda de la barra).
  let conversations = $state([]);
  let show_conversations = $state(false);
  let conversations_loading = $state(false);
  let conversations_error = $state(null);
  let switching_conversation = $state(false);
  // Cursor de teclado (j/k/arrows) y confirmación de borrado (d) — mismo
  // esquema de navegación que la sección de archivos del explorer.
  let conv_cursor = $state(0);
  let conv_delete_confirm = $state(null);
  let conversations_panel_ref = $state(null);

  let pending_model = $state(null);
  let abort_controller = $state(null);
  let input_ref = $state(null);
  let input_spacer_ref = $state(null);
  let base_line_h = 0; // altura de 1 línea del textarea (medida al montar)
  let models_error = $state(null);
  let agents_error = $state(null);
  let user_interacted = $state(false);
  let show_debug = $state(false);
  let last_switch_response = $state(null);
  let backend_flavor = $state("pi");
  let switching_backend = $state(false);
  // STT por voxtype (mic del chat)
  let mic_recording = $state(false);
  let mic_transcribing = $state(false);
  let mic_starting = $state(false);

  function save() {
    save_scope(active_scope);
  }

  async function toggle_mic() {
    if (mic_transcribing || mic_starting) return; // un solo frente a la vez

    if (!mic_recording) {
      // Empezar a grabar — UI optimista: rojo instantáneo; revertimos si falla
      mic_starting = true;
      mic_recording = true;
      try {
        const res = await fetch("/api/stt/voxtype/start", { method: "POST" });
        const data = await res.json();
        if (!data.ok) {
          mic_recording = false;
          warnings = [...warnings, data.error ?? "No se pudo iniciar la grabación"];
        }
      } catch (e) {
        mic_recording = false;
        warnings = [...warnings, `Mic falló: ${e?.message ?? e}`];
      } finally {
        mic_starting = false;
      }
    } else {
      // Parar → esperar transcripción → texto al input
      mic_recording = false;
      mic_transcribing = true;
      try {
        const res = await fetch("/api/stt/voxtype/stop", { method: "POST" });
        const data = await res.json();
        if (data.ok) {
          const text = data.data?.text ?? "";
          if (text) {
            input_value = input_value ? input_value.trimEnd() + " " + text : text;
          } else {
            warnings = [...warnings, "Grabación vacía (no se detectó voz)"];
          }
        } else {
          warnings = [...warnings, data.error ?? "No se pudo transcribir"];
        }
      } catch (e) {
        warnings = [...warnings, `Transcripción falló: ${e?.message ?? e}`];
      } finally {
        mic_transcribing = false;
      }
    }
  }

  async function newConversation() {
    try {
      await fetch("/api/chat/new-session", { method: "POST" });
    } catch {}
    messages = [];
    warnings = [];
    show_warnings = false;
    real_context_percent = null;
    real_context_window = null;
    // Reset solo del scope activo: las demás conversaciones quedan intactas.
    localStorage.removeItem(scope_key(active_scope));
    user_interacted = false;
  }

  $effect(() => {
    messages.length;
    const last = messages[messages.length - 1];
    last?.content;
    if (list_ref) {
      list_ref.scrollTop = list_ref.scrollHeight;
    }
  });

  // === Procesamiento de eventos del stream del turno (compartido entre el
  // send directo y el re-attach tras volver de otra sección) ===

  // Asegura que haya un mensaje assistant "en vuelo" para acumular tokens:
  // en el send ya existe (placeholder); en el re-attach hay que crearlo tras
  // el reconcile (la historia no incluye el mensaje que pi está generando).
  function ensure_assistant_target() {
    if (messages.length === 0 || messages[messages.length - 1].role !== "assistant") {
      messages = [...messages, { role: "assistant", content: "" }];
    }
  }

  // Procesa un evento data del turno. Devuelve "done"/"idle" para que la
  // bomba sepa que el turno terminó (o que no había turno activo).
  function process_sse_data(data) {
    switch (data.type) {
      case "token":
        ensure_assistant_target();
        messages[messages.length - 1].content += data.content;
        set_visual("talking");
        save();
        break;
      case "tool_call":
        ensure_assistant_target();
        messages[messages.length - 1].content += `\n\n[${data.name}]`;
        set_visual("thinking");
        save();
        break;
      case "warning":
        warnings = [...warnings, data.message];
        break;
      case "error":
        ensure_assistant_target();
        messages[messages.length - 1].content += `\n\nError: ${data.message}`;
        set_visual("confused");
        save();
        break;
      case "session_stats":
        try {
          const raw = JSON.parse(data.json);
          const ctx = raw.data?.contextUsage;
          if (ctx?.percent !== undefined) {
            // pi manda el percent TOTAL de la sesión: asignar, no acumular
            real_context_percent = ctx.percent;
          }
          if (ctx?.contextWindow) {
            real_context_window = ctx.contextWindow;
          }
        } catch {}
        break;
      case "attached": {
        // Turno activo en el backend: el cache puede tener el mensaje
        // assistant PARCIAL del turno (lo que llegó antes de irnos de la
        // sección) — el replay reconstruye el turno COMPLETO desde el
        // buffer, así que soltamos el parcial para no duplicar la cabeza.
        if (messages.length && messages[messages.length - 1].role === "assistant") {
          messages = messages.slice(0, -1);
        }
        set_visual("thinking");
        break;
      }
      case "done":
        return "done";
      case "idle":
        return "idle";
    }
    return null;
  }

  // Bomba del stream SSE: lee hasta ver done/idle o el cierre de conexión.
  async function pump_sse(res) {
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() || "";
      for (const line of lines) {
        if (!line.startsWith("data: ")) continue;
        try {
          const outcome = process_sse_data(JSON.parse(line.slice(6)));
          if (outcome) return outcome;
        } catch (e) {
          console.error("[chat] SSE parse error:", e, "line:", line);
        }
      }
    }
    return "closed";
  }

  // Cierre de turno: shared entre el send directo y el re-attach.
  function finish_turn() {
    if (messages.length && !messages[messages.length - 1].content.trim()) {
      messages[messages.length - 1].content = "(empty response)";
    }

    // Fin del turno: vuelve a stand-by, salvo que haya quedado en confused
    // (ese estado es sticky hasta el próximo mensaje).
    if (agent_visual === "thinking" || agent_visual === "talking") {
      set_visual("standby");
    }

    abort_controller = null;
    streaming = false;
    save();

    if (pending_model !== null) {
      // Fase 3: el backend aplicó el modelo encolado al terminar el turno
      // (y persistió el .md) — reflejarlo en la vista sin re-enviar.
      current_model = pending_model;
      pending_model = null;
    }

    // Foco de vuelta al input para escribir seguido
    input_ref?.focus();
  }

  async function send() {
    const text = input_value.trim();
    if (!text || streaming) return;

    // Slash-commands al estilo del CLI de pi. /compact: resume el contexto
    // de la sesión del agente activo (manual — pi también auto-compacta).
    if (text === "/compact") {
      input_value = "";
      // Feedback inmediato: el resume tarda decenas de segundos y el
      // servicio queda LIBRE mientras tanto (el RPC corre fuera del lock).
      warnings = [...warnings, "Compactando el contexto… (puede tardar ~40s)"];
      const scope_at_start = active_scope;
      try {
        const res = await fetch("/api/chat/compact", { method: "POST" });
        const data = await res.json();
        if (data.ok) {
          warnings = [...warnings, "Contexto compactado ✓"];
          // La compaction reescribió la sesión (resumen + cola): traer la
          // verdad nueva y refrescar el percent — SOLO si seguimos viendo
          // al mismo agente (durante el compact se pudo swichear).
          if (active_scope === scope_at_start) {
            await reconcile_messages();
            await load_stats();
          }
        } else {
          warnings = [...warnings, data.error ?? "Compact falló"];
        }
      } catch (e) {
        warnings = [...warnings, `Compact falló: ${e?.message ?? e}`];
      }
      input_ref?.focus();
      return;
    }

    messages = [...messages, { role: "user", content: text }];
    save();
    input_value = "";
    streaming = true;

    messages = [...messages, { role: "assistant", content: "" }];
    save();

    abort_controller = new AbortController();

    try {
      const res = await fetch("/api/chat/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: text }),
        signal: abort_controller.signal,
      });

      if (
        !res.ok ||
        !res.headers.get("Content-Type")?.includes("text/event-stream")
      ) {
        const err_text = await res.text();
        messages[messages.length - 1].content = `Error del servidor:\n${err_text}`;
        save();
        streaming = false;
        return;
      }

      await pump_sse(res);
    } catch (e) {
      if (e?.name === 'AbortError') {
        // Abort por cambio de sección (cleanup) o cancel manual: el turno
        // SIGUE VIVO en el backend (el forwarder drena siempre) — NO lo
        // cerremos acá como fallido (escribía "(empty response)" al cache
        // aunque el agente seguía trabajando). Al volver, /chat/stream
        // re-attacha y el reconcile trae la verdad.
        return;
      }
      ensure_assistant_target();
      messages[messages.length - 1].content += `\n\nConnection error: ${e.message}`;
      save();
    }

    finish_turn();
  }

  async function cancel() {
    // Abortar el turno en pi/little-coder de verdad; sin esto solo cortábamos
    // la conexión del frontend y el modelo seguía pensando igual.
    try {
      await fetch("/api/chat/abort", { method: "POST" });
    } catch {}
    abort_controller?.abort();
    abort_controller = null;
    streaming = false;
    set_visual("standby");
    input_ref?.focus();
  }

  function handle_keydown(e) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  // Auto-resize del textarea: crece con el contenido hasta ~3 líneas, después
  // scrollea adentro. El spacer in-flow refleja SOLO el excedente sobre la
  // primera línea, así el contenedor conserva sus proporciones originales con
  // 1 línea y crece hacia arriba a partir de la segunda.
  $effect(() => {
    input_value;
    if (input_ref) {
      input_ref.style.height = "auto";
      if (!base_line_h) base_line_h = input_ref.scrollHeight;
      const h = Math.min(input_ref.scrollHeight, base_line_h * 3);
      input_ref.style.height = h + "px";
      if (input_spacer_ref) {
        input_spacer_ref.style.height = Math.max(0, h - base_line_h) + "px";
      }
    }
  });

  function handle_tab_keydown(e) {
    if (e.key === "Tab" && sub_agents.length > 0) {
      e.preventDefault();
      if (!current_agent) {
        select_agent(e.shiftKey ? sub_agents[sub_agents.length - 1].name : sub_agents[0].name);
        return;
      }
      const idx = sub_agents.findIndex(a => a.name === current_agent);
      if (e.shiftKey) {
        select_agent(sub_agents[idx <= 0 ? sub_agents.length - 1 : idx - 1].name);
      } else {
        select_agent(sub_agents[idx >= sub_agents.length - 1 ? 0 : idx + 1].name);
      }
    }
  }

  async function load_models() {
    try {
      const res = await fetch("/api/models/list");
      const data = await res.json();
      models = data.data || [];
      models_error = null;
      if (models.length > 0) {
        const default_model = models.find((m) => m.default);
        if (!user_interacted) {
          current_model = default_model ? default_model.name : models[0].name;
        }
      }
    } catch (e) {
      models_error = e?.message ?? "Error desconocido al cargar modelos";
    }
  }

  async function load_sub_agents() {
    try {
      const res = await fetch("/api/sub-agents/list");
      const data = await res.json();
      sub_agents = data.data || [];
      agents_error = null;
    } catch (e) {
      agents_error = e?.message ?? "Error desconocido al cargar subagentes";
      sub_agents = [];
    }
    try {
      const res = await fetch("/api/sub-agents/builtins");
      const data = await res.json();
      builtins = data.data || [];
      agents_error = null;
    } catch (e) {
      agents_error = e?.message ?? "Error desconocido al cargar builtins";
      builtins = [];
    }
  }

  async function load_backend_flavor() {
    try {
      const res = await fetch("/api/chat/backend");
      const data = await res.json();
      if (data.ok && data.data?.backend) {
        backend_flavor = data.data.backend;
      }
    } catch {}
  }

  async function select_backend(name) {
    if (!name || name === backend_flavor || switching_backend || streaming) return;
    switching_backend = true;
    try {
      const res = await fetch("/api/chat/switch-backend", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ backend: name }),
      });
      const data = await res.json();
      if (data.ok) {
        backend_flavor = data.data.backend;
      } else {
        warnings = [...warnings, data.error ?? "No se pudo cambiar de backend"];
      }
    } catch (e) {
      warnings = [...warnings, `Cambio de backend falló: ${e?.message ?? e}`];
    }
    switching_backend = false;
  }

  // Picker de conversaciones: lista las sesiones del proyecto actual (leo
  // directo del storage de pi) y permite retomar cualquiera — incluidas las
  // huérfanas de switches de proyecto previos al fix del mapa por-proyecto.
  async function load_conversations() {
    conversations_loading = true;
    conversations_error = null;
    try {
      const res = await fetch("/api/chat/conversations");
      const data = await res.json();
      if (data.ok && Array.isArray(data.data?.conversations)) {
        conversations = data.data.conversations;
      } else {
        conversations_error = data.error ?? "No se pudo listar";
      }
    } catch (e) {
      conversations_error = `Lista falló: ${e?.message ?? e}`;
    } finally {
      conversations_loading = false;
    }
  }

  function toggle_conversations() {
    show_conversations = !show_conversations;
    if (show_conversations) {
      conv_cursor = 0;
      conv_delete_confirm = null;
      load_conversations().then(async () => {
        await tick();
        conversations_panel_ref?.focus();
      });
    }
  }

  // Navegación por teclado del panel — mismos movimientos que el fs:
  // k/↑ y j/↓ mueven el cursor, l/→ retoma la conversación, d pide
  // confirmación para eliminar (diálogo Si/No del canal del explorer),
  // Escape cierra. El panel tiene el foco mientras se navega, así los keys
  // no le roban la escritura al input del chat.
  function handle_conversations_keydown(e) {
    // El diálogo Si/No maneja sus propios keys (burbujean hasta acá): ignorar.
    if (conv_delete_confirm) return;
    const n = conversations.length;
    if (!n) return;
    const handled = ["j", "k", "l", "d", "D", "ArrowDown", "ArrowUp", "ArrowRight", "Escape"];
    if (!handled.includes(e.key)) return;
    e.preventDefault();
    switch (e.key) {
      case "k":
      case "ArrowUp":
        conv_cursor = (conv_cursor - 1 + n) % n;
        conv_scroll_cursor();
        break;
      case "j":
      case "ArrowDown":
        conv_cursor = (conv_cursor + 1) % n;
        conv_scroll_cursor();
        break;
      case "l":
      case "ArrowRight":
        pick_conversation(conversations[conv_cursor]?.id);
        break;
      case "d":
      case "D": {
        const c = conversations[conv_cursor];
        if (c) {
          conv_delete_confirm = c;
          // El cartel Si/No vive en el MONITOR (mismo lugar y animación que
          // los diálogos del explorer), no sobre la lista.
          active_channel = {
            component: ExplorerChannel,
            props: {
              mode: "confirm",
              owner_section: "chat",
              icon: "ri:chat-delete-line",
              name: c.preview ?? c.id.slice(0, 12),
              on_confirm: () => delete_conversation(c.id),
              on_cancel: () => {},
              on_close: () => {
                conv_delete_confirm = null;
                // El diálogo tenía el foco: devolvérselo al picker para
                // poder seguir borrando sin tocar el mouse.
                conversations_panel_ref?.focus();
              },
            },
          };
        }
        break;
      }
      case "Escape":
        show_conversations = false;
        input_ref?.focus();
        break;
    }
  }

  function conv_scroll_cursor() {
    conversations_panel_ref
      ?.querySelector(`[data-cindex="${conv_cursor}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }

  async function delete_conversation(id) {
    try {
      const res = await fetch("/api/chat/delete-conversation", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id }),
      });
      const data = await res.json();
      if (!data.ok) {
        warnings = [...warnings, data.error ?? "No se pudo eliminar la conversación"];
        return;
      }
      await load_conversations();
      if (conv_cursor >= conversations.length) {
        conv_cursor = Math.max(0, conversations.length - 1);
      }
      // Mantener el flujo de borrado continuo: foco de vuelta al picker.
      conversations_panel_ref?.focus();
    } catch (e) {
      warnings = [...warnings, `Eliminar conversación falló: ${e?.message ?? e}`];
    }
  }

  async function pick_conversation(id) {
    if (switching_conversation || switching || streaming) return;
    switching_conversation = true;
    try {
      const res = await fetch("/api/chat/select-conversation", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ id }),
      });
      const data = await res.json();
      if (!data.ok) {
        warnings = [...warnings, data.error ?? "No se pudo cambiar de conversación"];
        return;
      }
      show_conversations = false;

      // La conversación puede pertenecer a OTRO agente (o al chat default):
      // el backend ya cambió la persona junto con ella — reflejar el cambio en
      // la UI igual que en select_agent (icono, modelo, scope del cache).
      const new_agent = data.data?.agent ?? null;
      if (new_agent !== current_agent) {
        save_scope(active_scope);
        current_agent = new_agent;
        active_scope = new_agent || "default";
        const agent = new_agent ? sub_agents.find(a => a.name === new_agent) : null;
        current_icon = agent?.icon || null;
        if (agent?.model) {
          const resolved = models.find(m =>
            m.name.localeCompare(agent.model, undefined, { sensitivity: 'base' }) === 0
          );
          current_model = resolved ? resolved.name : agent.model;
        } else {
          const dm = models.find(m => m.default);
          if (dm) current_model = dm.name;
        }
      }

      // Cache instantáneo + reconciliación con la conversación real
      messages = load_scope(active_scope);
      await reconcile_messages();

      // Percent de la conversación nueva (resetea el de la anterior)
      real_context_percent = null;
      real_context_window = null;
      await load_stats();

      // Feedback visual del swap de contexto + foco en el input
      flash_static();
      await tick();
      input_ref?.focus();
    } catch (e) {
      warnings = [...warnings, `Cambio de conversación falló: ${e?.message ?? e}`];
    } finally {
      switching_conversation = false;
    }
  }

  function fmt_conv_date(c) {
    const d = typeof c === "number" ? new Date(c) : new Date(String(c).replace(" ", "T"));
    if (isNaN(d)) return "?";
    return d.toLocaleDateString(undefined, {
      day: "2-digit", month: "short",
      hour: "2-digit", minute: "2-digit",
    });
  }

  function toggle_dropdown() {
    show_dropdown = !show_dropdown;
  }

  async function select_model(name) {
    user_interacted = true;
    if (!name || name === current_model) {
      show_dropdown = false;
      return;
    }
    show_dropdown = false;

    switching = true;
    try {
      const res = await fetch("/api/chat/switch-model", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      const data = await res.json();
      if (data.ok) {
        const info = data.data ?? {};
        if (info.queued) {
          // Fase 3: el agente está trabajando — el backend encola el cambio
          // y lo aplica (y persiste al .md) cuando SU turno termina, aunque
          // ya estemos viendo a otro agente. El dot pulsante lo indica.
          pending_model = name;
          switching = false;
          return;
        }
        current_model = info.model ?? name;
        pending_model = null;
        // NOTA: ya no se mata el agente activo al cambiar el modelo manualmente:
        // la personalidad sigue viva en la sesión del scope (cambio de modelo ≠ cambio de conversación).
      }
    } catch {
      // Si falla, el nombre no se actualiza
    }
    switching = false;
  }

  function toggle_agent_dropdown() {
    show_agent_dropdown = !show_agent_dropdown;
    if (show_agent_dropdown) {
      show_dropdown = false;
    }
  }

  // La sesión de pi es la fuente de verdad: reemplaza el cache de la UI con el
  // historial real de la sesión activa (sobrevive restarts, borrados de
  // localStorage y conversaciones externas vía pi --resume).
  async function reconcile_messages() {
    try {
      const res = await fetch("/api/chat/messages");
      const data = await res.json();
      if (data.ok && Array.isArray(data.data?.messages)) {
        messages = data.data.messages
          .filter(m => m.role === "user" || m.role === "assistant")
          .map(m => {
            let content = Array.isArray(m.content)
              ? m.content.filter(c => c?.type === "text").map(c => c.text).join("")
              : String(m.content ?? "");
            // El backend inyecta el prompt del agente como prefijo del primer
            // mensaje de una sesión nueva; la UI solo muestra el mensaje real.
            content = content.replace(/^\[System instructions\]\n[\s\S]*?\n\n---\n\n/, "");
            return { role: m.role, content };
          })
          .filter(m => m.content.trim().length > 0);
        save();
      } else {
        // Sin esto, un backend muerto a mitad de switch dejaba el cache stale
        // mostrado en silencio (solo console.error) y parecía "el historial
        // no cargó" hasta recargar la página.
        warnings = [...warnings, data.error ?? "No se pudo sincronizar el historial con la sesión real"];
      }
    } catch (e) {
      console.error("[chat] reconcile_messages:", e);
      warnings = [...warnings, "No se pudo sincronizar el historial con la sesión real"];
    }
  }

  // Percent real de la sesión activa: lo calcula pi contra la ventana de
  // contexto real del modelo en uso (el mismo número que muestra la TUI).
  async function load_stats() {
    try {
      const res = await fetch("/api/chat/stats");
      const data = await res.json();
      if (data.ok && data.data?.contextUsage) {
        if (data.data.contextUsage.percent !== undefined) {
          real_context_percent = data.data.contextUsage.percent;
        }
        if (data.data.contextUsage.contextWindow) {
          real_context_window = data.data.contextUsage.contextWindow;
        }
      }
    } catch {
      // sin stats, el indicador cae al estimate local
    }
  }

  async function select_agent(name) {
    user_interacted = true;
    show_agent_dropdown = false;
    if (name === current_agent) return;

    // pending_model es visual del scope VISTO: el pendiente real vive en el
    // backend (aplica aunque estemos en otro agente). No filtrarlo.
    pending_model = null;

    // Multi-agente paralelo: el turno del agente actual SIGUE corriendo en
    // el backend (drena siempre) — solo soltamos SU conexión para que su
    // pump no escriba sobre la vista del agente nuevo. Al volver a ese
    // agente, /chat/stream?agent=X re-attacha su turno donde estaba.
    if (streaming) {
      abort_controller?.abort();
      abort_controller = null;
      streaming = false;
      set_visual("standby");
    }

    try {
      // Selector unificado: agente por nombre, o null para el chat default.
      // Cada scope tiene su propia sesión; el backend la retoma/crea.
      const res = await fetch("/api/chat/select-scope", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ agent: name }),
      });
      const data = await res.json();
      if (data.ok) {
        // Swap de conversación: guardar la actual bajo su scope y cargar la nueva
        save_scope(active_scope);
        current_agent = name;
        active_scope = name || "default";

        // Feedback visual del agente/modelo (el backend ya aplicó todo)
        const agent = name ? sub_agents.find(a => a.name === name) : null;
        current_icon = agent?.icon || null;
        if (agent?.model) {
          const resolved = models.find(m =>
            m.name.localeCompare(agent.model, undefined, { sensitivity: 'base' }) === 0
          );
          current_model = resolved ? resolved.name : agent.model;
        } else {
          const dm = models.find(m => m.default);
          if (dm) current_model = dm.name;
        }
        last_switch_response = data.data;
        if (data.data.warning) warnings = [...warnings, data.data.warning];

        // Percent de la sesión NUEVA (resetea el del agente anterior)
        real_context_percent = null;
        real_context_window = null;

        // Vista del scope NUEVO: cache → attach a SU turno (si está
        // trabajando) → reconcile con su historial real.
        messages = load_scope(active_scope);
        await sync_scope_view();

        // Estática de cambio de agente + visual nuevo en el monitor
        if (agent?.assets) flash_static();
        await tick();
        input_ref?.focus();
      } else {
        // Sin este else, un fallo (p. ej. set_model RPC inválido) era invisible:
        // el Tab parecía no hacer nada.
        warnings = [...warnings, data.error ?? "No se pudo cambiar de agente"];
      }
    } catch (e) {
      warnings = [...warnings, `Cambio de agente falló: ${e?.message ?? e}`];
    }
  }

  function handle_debug_keydown(e) {
    if (e.ctrlKey && e.shiftKey && e.key === 'D') {
      e.preventDefault();
      show_debug = !show_debug;
    }
  }

  function handle_click_outside(e) {
    if (
      show_dropdown &&
      dropdown_container &&
      !dropdown_container.contains(e.target)
    ) {
      show_dropdown = false;
    }
    if (
      show_agent_dropdown &&
      agent_dropdown_container &&
      !agent_dropdown_container.contains(e.target)
    ) {
      show_agent_dropdown = false;
    }
  }

  // El proyecto cambió (switch desde explorer/settings): re-sincronizar TODO
  // con el backend nuevo. El switch del explorer ya terminó cuando este evento
  // llega, así que las consultas ven la sesión del proyecto correcto — cubre la
  // carrera de un init que corrió DURANTE el switch y quedó con estado viejo.
  async function handle_project_switched() {
    await init();
  }

  onMount(() => {
    // Foco automático: al entrar a la sección (se remonta), el cursor queda listo para escribir.
    input_ref?.focus();
    document.addEventListener("click", handle_click_outside);
    document.addEventListener("keydown", handle_tab_keydown);
    document.addEventListener("keydown", handle_debug_keydown);
    window.addEventListener("weztcode:project-switched", handle_project_switched);
    return () => {
      document.removeEventListener("click", handle_click_outside);
      document.removeEventListener("keydown", handle_tab_keydown);
      document.removeEventListener("keydown", handle_debug_keydown);
      window.removeEventListener("weztcode:project-switched", handle_project_switched);
      // Al cambiar de sección cortamos la conexión SSE — es SEGURO: el backend
      // drena el turno siempre (pi sigue trabajando en background) y al
      // volver, /chat/stream re-attacha desde el buffer sin perder eventos.
      abort_controller?.abort();
    };
  });

  function estimateTokens(text) {
    return Math.ceil(text.length / 4);
  }

  let used_tokens = $derived(
    messages.reduce((sum, msg) => sum + estimateTokens(msg.content), 0),
  );
  let context_limit = $derived(
    models.find((m) => m.name === current_model)?.max_context
      ?? real_context_window
      ?? 4096,
  );
  let context_percent = $derived(
    context_limit > 0 ? (used_tokens / context_limit) * 100 : 0,
  );
  let indicator = $derived(
    Math.round(real_context_percent ?? context_percent),
  );

  // Sincroniza la vista con el scope ACTIVO. Attach primero (con
  // ?agent=<scope>): durante un turno activo, el RPC de get_messages se muere
  // de hambre esperando el stdout de pi (lo consume el reader del stream) y
  // congelaría el lock del servicio. Con turno activo, el stream ES la
  // verdad (replay + live); reconcile recién al terminar — o directamente si
  // está idle.
  async function sync_scope_view() {
    let turn_res = null;
    try {
      abort_controller = new AbortController();
      turn_res = await fetch(
        `/api/chat/stream?agent=${encodeURIComponent(active_scope)}`,
        { signal: abort_controller.signal },
      );
    } catch (e) {
      abort_controller = null;
      console.error("[chat] re-attach fetch:", e);
    }

    const is_sse =
      turn_res?.ok &&
      turn_res.headers.get("Content-Type")?.includes("text/event-stream");
    if (is_sse) {
      // El primer evento del stream decide: "attached" (turno activo) o
      // "idle". El pump procesa ambos — ver process_sse_data.
      streaming = true;
      const outcome = await pump_sse(turn_res);
      if (outcome === "idle") {
        streaming = false;
        abort_controller = null;
        await reconcile_messages();
        await load_stats();
      } else {
        // El turno terminó (re-attach consumió su final): sincronizar con
        // la verdad de pi y refrescar el percent del contexto.
        finish_turn();
        await reconcile_messages();
        await load_stats();
      }
    } else {
      abort_controller = null;
      await reconcile_messages();
      await load_stats();
    }
  }

  async function init() {
    await Promise.all([load_models(), load_sub_agents()]);

    // Scope de arranque: el backend bootea directo con la sesión del agente
    // default (o del chat default). Preguntamos la verdad en vez de asumir.
    try {
      const res = await fetch("/api/chat/active-agent");
      const data = await res.json();
      if (data.ok && data.data) {
        current_agent = data.data.agent || null;
        current_project = data.data.project || "";
      }
    } catch {}

    active_scope = current_agent || "default";
    const agent = current_agent
      ? sub_agents.find(a => a.name === current_agent)
      : null;
    current_icon = agent?.icon || null;
    if (agent?.model) {
      const resolved = models.find(m =>
        m.name.localeCompare(agent.model, undefined, { sensitivity: 'base' }) === 0
      );
      current_model = resolved ? resolved.name : agent.model;
    }

      // Vista del scope: attach a SU turno en curso (multi-agente: cada
    // agente re-attacha el suyo — nunca el de otro) o reconcile si idle.
    messages = load_scope(active_scope);
    await sync_scope_view();

    // Aparición del agente al bootear: estática + visual en el monitor
    const boot_agent = current_agent
      ? sub_agents.find(a => a.name === current_agent)
      : null;
    if (boot_agent?.assets) flash_static();
  }

  init();
</script>

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div
    class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0"
  >
    <button
      class="text-accent-detail/70 hover:text-accent-detail transition-colors {show_conversations ? 'text-accent-detail' : ''}"
      onclick={toggle_conversations}
      disabled={streaming || switching || switching_conversation}
    >
      <Icon icon="ic:round-chat-bubble-outline" class="w-4 h-4" />
    </button>
    <span class="font-mono truncate flex items-center gap-1 text-print/50"></span>
    <div class="ml-auto flex items-center gap-2">
      {#if warnings.length > 0}
        <button
          class="relative text-sm px-2 text-print/50 hover:text-print transition-colors group"
          onclick={() => (show_warnings = !show_warnings)}
        >
          <Icon
            icon="mdi:alert-outline"
            class="w-4 h-4 z-10 group-hover:text-print"
          />
          <span
            class="absolute -top-1 -right-1 bg-back font-bold text-print/50 text-[10px] rounded-full w-3 h-3 flex items-center justify-center group-hover:text-print"
          >
            {warnings.length}
          </span>
        </button>
      {/if}



      <div class="relative" bind:this={dropdown_container}>
        <button
          class="px-1 font-mono text-xs {models_error ? 'text-red-400' : pending_model !== null ? 'text-accent-detail' : 'text-print/50 hover:text-print'} rounded hover:text-print transition-colors max-w-[120px] truncate"
          onclick={toggle_dropdown}
          disabled={switching}
        >
          {models_error ? `⚠ ${models_error}` : ($state.eager(current_model) || "sin modelos")}
          {#if pending_model !== null}
            <span class="ml-1 w-1.5 h-1.5 rounded-full bg-accent-detail animate-pulse"></span>
          {/if}
        </button>
        {#if show_dropdown}
          <div
            class="absolute top-full right-1 mt-4 z-50 min-w-full bg-back rounded-md shadow-lg py-1"
          >
            {#if models_error}
              <div class="px-3 py-1.5 text-xs text-red-500">{models_error}</div>
            {:else if models.length === 0}
              <div class="px-3 py-1.5 text-xs text-print/50">No hay modelos disponibles</div>
            {:else}
              {#each models as m}
                <button
                  class="font-semibold block w-full text-left text-sm px-3 py-1.5 whitespace-nowrap rounded-md
                       {m.name === $state.eager(current_model)
                    ? 'bg-accent-detail text-back transition-colors'
                    : 'text-print hover:bg-accent-detail/10 transition-colors'}"
                  onclick={() => select_model(m.name)}
                >
                  {m.name}
                </button>
              {/each}
            {/if}
          </div>
        {/if}
      </div>

      <button
        class="font-mono text-xs text-print/50 hover:text-print transition-colors"
        onclick={newConversation}
      >
        {indicator}%
      </button>
    </div>
  </div>

  {#if show_conversations}
    <!-- Mismo cuadro que el de advertencias; los items copian el estilo de la
         lista de modelos. Navegación de teclado igual al fs (k/j/l/d) — el
         panel toma el foco al abrirse para no pisar la escritura del chat. -->
    <div
      bind:this={conversations_panel_ref}
      tabindex="0"
      onkeydown={handle_conversations_keydown}
      class="mx-3 mb-2 p-2 rounded bg-back max-h-52 overflow-y-auto text-sm font-mono text-print outline-none"
    >
      {#if conversations_loading}
        <div class="px-3 py-1.5 text-xs text-print/50">Cargando</div>
      {:else if conversations_error}
        <div class="px-3 py-1.5 text-xs text-red-500">{conversations_error}</div>
      {:else}
        {#each conversations as c, index (c.id)}
          <button
            data-cindex={index}
            onmouseenter={() => (conv_cursor = index)}
            class="font-semibold block w-full text-left text-sm px-3 py-1.5 rounded-md transition-colors
                   {conv_cursor === index
                    ? 'bg-accent-detail text-back'
                    : 'text-print hover:bg-accent-detail/10'}"
            onclick={() => {
              conv_cursor = index;
              pick_conversation(c.id);
            }}
            disabled={switching_conversation || streaming || c.current}
          >
            <span class="block truncate">{c.preview ?? "(sin mensajes)"}</span>
            <span
              class="block font-normal text-[11px] truncate
                   {conv_cursor === index ? 'text-back/70' : 'text-print/50'}"
            >
              {fmt_conv_date(c.modified_ms || c.created)} · {c.agent ?? "default"} ·
              {c.messages} msgs{#if c.current} · actual{/if}
            </span>
          </button>
        {/each}
      {/if}
      <div class="px-1 text-[10px] text-print/70 border-t border-accent-detail/10 text-center">
        Conversaciones de este proyecto [{conversations.length}]
      </div>
      <div class="px-1 pb-1 mb-1 text-[10px] text-print/60 border-b border-accent-detail/10 text-center">
        k/j mover · l abrir · d borrar
      </div>
    </div>
  {/if}

  {#if show_warnings && warnings.length > 0}
    <div
      class="mx-3 mb-2 p-2 rounded bg-back max-h-40 overflow-y-auto text-sm font-mono text-print"
    >
      {#each warnings as w, i (i)}
        <div class="py-0.5 border-b border-accent-detail/10 last:border-0">
          ⚠ {w}
        </div>
      {/each}
    </div>
  {/if}

  <div class="flex-1 overflow-y-auto px-3 py-2 space-y-3" bind:this={list_ref}>
    {#if messages.length === 0}
      <div class="flex items-center justify-center h-full">
        <div class="text-center text-print/50">
          <Icon icon="mdi:thinking" class="w-20 h-20 mx-auto mb-3 opacity-50" />
          <p class="text-xl">Animate a crear</p>
        </div>
      </div>
    {:else}
      {#each messages as msg, i (i)}
        <div
          class={"flex " +
            (msg.role === "user" ? "justify-end" : "justify-start")}
        >
          <div
            class={"max-w-[85%] px-2 py-1 font-semibold rounded-lg text-lg whitespace-pre-wrap " +
              (msg.role === "user"
                ? "bg-accent-detail/95 text-back-deep"
                : "bg-back text-print")}
          >
            {#if msg.role === "assistant" && i === messages.length - 1 && streaming}
              {msg.content}<span class="animate-pulse">▌</span>
            {:else}
              {msg.content}
            {/if}
          </div>
        </div>
      {/each}
    {/if}
  </div>

  <div class="flex flex-col shrink-0">
    <div class="flex mt-auto pt-2 pb-2 w-full items-stretch relative">
      <div
        class="-mr-2 flex-1 relative px-5 py-6 bg-transparent rounded-l-lg flex items-center justify-center overflow-hidden group"
      >
        <div
          class="rounded-lg absolute inset-0 bg-back -translate-x-[4%] skew-x-[-20deg] origin-left"
        >
          <textarea
            rows="1"
            placeholder="Decime lo que pensás..."
            bind:value={input_value}
            bind:this={input_ref}
            onkeydown={handle_keydown}
            disabled={streaming}
            class="w-full skew-x-[20deg] pl-4 pr-2 bg-transparent rounded-lg text-lg text-print-contrast placeholder:text-print-contrast/50 outline-none resize-none overflow-y-auto"
          ></textarea>
        </div>

        <!-- Espejo in-flow de la altura del textarea: con 1 línea no aporta nada
             (el contenedor conserva el alto original de solo py-6) y a partir de
             la 2ª línea empuja el área hacia arriba por el excedente. -->
        <div class="w-0" bind:this={input_spacer_ref} aria-hidden="true"></div>

        <div class="absolute right-[25px] -bottom-1 z-50">
          <button
            class="{mic_recording ? 'text-red-400 animate-pulse' : 'text-print/50 hover:text-print'} active:scale-50 transition-transform p-2 -m-2"
            onclick={toggle_mic}
            disabled={mic_transcribing}
            title={mic_transcribing ? "Transcribiendo..." : mic_recording ? "Grabando — click para parar" : "Dictar por voz"}
          >
            <Icon
              icon={mic_transcribing ? "svg-spinners:bars-scale-fade" : "si:mic-detailed-fill"}
              class="w-4 h-4"
            />
          </button>
        </div>
      </div>

      <div
        class="absolute left-0 bottom-2 z-50"
        bind:this={agent_dropdown_container}
      >
        <!-- Multi-agente: SIEMPRE clickeable — cambiar de agente mientras
             uno trabaja es el feature, no un peligro (fase 2). -->
        <button
          class="flex items-center gap-1 px-2 py-0.5 text-xs font-semibold font-mono {agents_error ? 'text-red-400' : pending_model !== null ? 'text-accent-detail' : 'text-print/50 hover:text-print'} transition-colors rounded"
          onclick={toggle_agent_dropdown}
        >
          <Icon icon={streaming ? "svg-spinners:bars-scale-fade" : current_icon || "simple-icons:pi"} class="w-4 h-4" />
          {agents_error ? `⚠ ${agents_error}` : ($state.eager(current_agent) || "default")}
          <!-- Fase 3: cambio de modelo encolado — aplica cuando el agente
               termine su turno. El mismo indicador intermitente que tenía
               el switch pendiente, ahora para la configuración del agente. -->
          {#if pending_model !== null}
            <span class="ml-1 w-2 h-2 rounded-full bg-accent-detail animate-pulse"></span>
          {/if}
        </button>
        {#if show_agent_dropdown}
          <div
            class="absolute bottom-full left-1 mb-5 z-50 min-w-[140px] bg-back rounded-md shadow-lg py-1"
          >
            {#if agents_error}
              <div class="px-3 py-1.5 text-xs text-red-500">{agents_error}</div>
            {:else}
              {#each sub_agents as agent}
                <button
                  class="font-semibold block w-full text-left text-sm px-3 py-1.5 whitespace-nowrap rounded-md
                       {agent.name === $state.eager(current_agent)
                    ? 'bg-accent-detail text-back trasition-colors'
                    : 'text-print hover:bg-accent-detail/10 transition-colors'}"
                  onclick={() => select_agent(agent.name)}
                >
                  {agent.name}
                  {#if agent.engine}
                    <span
                      class="ml-1 px-1 text-[10px] rounded border {agent.name === $state.eager(current_agent)
                        ? 'border-back/50 text-back/80'
                        : 'border-accent-detail/40 text-print/50'}"
                    >{agent.engine === "little-coder" ? "lc" : agent.engine}</span>
                  {/if}
                </button>
              {/each}
              <hr class="border-accent-detail/10 my-1" />
              <button
                class="font-semibold block w-full text-left text-sm px-3 py-1.5 rounded
                       {current_agent === null
                  ? 'bg-accent-detail text-back-deep trasition-colors'
                  : 'text-print hover:bg-accent-detail/10 hover:text-accent/70 transition-colors'}"
                onclick={() => select_agent(null)}
              >
                default
              </button>
            {/if}
          </div>
        {/if}
      </div>

      <div
        class="relative self-end h-12 pl-5 pr-3 -ml-2 bg-transparent rounded-r-lg flex items-center justify-center overflow-hidden group"
      >
        <button
          class="rounded-lg absolute inset-0 bg-accent-detail translate-x-[14%] skew-x-[-20deg] origin-right"
          aria-label={mic_recording ? "Parar grabación" : streaming ? "Cancelar" : "Enviar"}
          onclick={mic_recording ? toggle_mic : streaming ? cancel : send}
        ></button>

        <Icon
          icon={mic_recording ? "mdi:square" : streaming ? "mdi:circle" : "mingcute:navigation-fill"}
          class="pointer-events-none text-back-deep w-6 h-6 relative z-10 transition-transform group-active:scale-75"
        />
      </div>
    </div>
  </div>
</div>

{#if show_debug}
  <div
    class="fixed bottom-2 right-2 z-[999] bg-back/95 border border-accent-detail/30 rounded-md p-3 text-[10px] font-mono text-print shadow-lg max-w-xs max-h-60 overflow-y-auto"
  >
    <div class="font-bold text-xs mb-1 text-accent-detail">Debug</div>
    <div class="space-y-0.5">
      <div>agent: <span class="text-accent-detail">{JSON.stringify($state.eager(current_agent))}</span></div>
      <div>model: <span class="text-accent-detail">{JSON.stringify($state.eager(current_model))}</span></div>
      <div>model_match: <span class="text-accent-detail">{models.find(m => m.name === $state.eager(current_model))?.name ?? "NO MATCH"}</span></div>
      <div>last_switch_resp: <span class="text-accent-detail">{JSON.stringify($state.eager(last_switch_response))}</span></div>
      <div>user_interacted: <span class="text-accent-detail">{JSON.stringify($state.eager(user_interacted))}</span></div>
      <div>models: {$state.eager(models).length} | agents: {$state.eager(sub_agents).length}</div>
    </div>
  </div>
{/if}

<style>
  button:focus-visible {
    outline: none;
  }
</style>
