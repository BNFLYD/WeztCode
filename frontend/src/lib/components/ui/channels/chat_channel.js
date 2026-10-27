// Canal del chat: dueño de sus vistas del monitor (visual del agente
// activo + diálogo Si/No de borrado de conversación). La sección solo
// dispara signals ({ type: 'agent' | 'confirm', ... }); el canal mapea
// cada signal a una vista { key, component, props } y emite el update
// al monitor.
//
// Estados vivos: los estados del MISMO agente (standby|talking|
// thinking|confused) comparten la key `agent:{nombre}` → el monitor los
// swapea en caliente, sin estática. Cambiar de agente es un update
// (estática → visual nuevo).
//
// Precedencia: mientras el diálogo está abierto, las signals de agente
// solo actualizan el estado interno (quedan "detrás"); al cerrarse el
// diálogo se emite el estado fresco del agente.

import { emit_to_monitor } from "../monitor_bus";
import { get_active_section, on_section_change } from "$lib/sections";
import AgentView from "./agent_view.svelte";
import DialogView from "./dialog_view.svelte";

// { name, has_assets, visual } | null
let agent = null;
let dialog = false;

// El canal vive solo dentro de su sección: al salir de chat, su estado
// muere.
on_section_change((id) => {
  if (id !== "chat") {
    agent = null;
    dialog = false;
  }
});

function emit_agent_view() {
  const view =
    agent?.name && agent.has_assets
      ? {
          key: `agent:${agent.name}`,
          component: AgentView,
          props: { state: agent.visual, agent_name: agent.name },
        }
      : null; // sin agente con assets → idle (la onda)
  emit_to_monitor({ type: "view", view });
}

export function signal(event) {
  if (get_active_section() !== "chat") return;

  switch (event.type) {
    case "agent": {
      agent = {
        name: event.name,
        has_assets: event.has_assets,
        visual: event.visual,
      };
      if (dialog) return; // el diálogo gana; el estado fresco sale al cerrar
      emit_agent_view();
      return;
    }

    case "confirm": {
      dialog = true;
      emit_to_monitor({
        type: "view",
        view: {
          key: `confirm:${event.name}`,
          component: DialogView,
          props: {
            icon: event.icon,
            name: event.name,
            on_confirm: event.on_confirm,
            on_cancel: event.on_cancel,
            // El canal devuelve la pantalla al estado del agente (o idle)
            // y recién después deja que la sección resetee su estado.
            on_close: () => {
              dialog = false;
              emit_agent_view();
              event.on_close?.();
            },
          },
        },
      });
      return;
    }
  }
}
