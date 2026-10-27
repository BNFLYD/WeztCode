// Canal del explorer: dueño de sus vistas del monitor (preview de
// imágenes + diálogo Si/No de borrado). La sección solo dispara
// signals ({ type: 'preview' | 'confirm' | 'clear', ... }); el canal
// mapea cada signal a una vista { key, component, props } y emite el
// update al monitor. La estática de transición la maneja el monitor,
// no este canal.
//
// Precedencia: mientras el diálogo está abierto, preview/clear se
// ignoran — el diálogo cierra SOLO con su propia resolución (Si/No/
// Escape/X). Si el cursor se mueve en ese lapso, la sección re-sincro-
// niza el preview al cerrarse el diálogo.

import { emit_to_monitor } from "../monitor_bus";
import { get_active_section, on_section_change } from "$lib/sections";
import PreviewView from "./preview_view.svelte";
import DialogView from "./dialog_view.svelte";

// null = idle | { type: 'preview' } | { type: 'confirm' }
let state = null;

// El canal vive solo dentro de su sección: al salir de explorer, su
// estado muere (si quedara un diálogo pendiente, no podría bloquear
// signals la próxima vez que se entra).
on_section_change((id) => {
  if (id !== "explorer") state = null;
});

export function signal(event) {
  if (get_active_section() !== "explorer") return;

  switch (event.type) {
    case "preview": {
      if (state?.type === "confirm") return;
      state = { type: "preview" };
      emit_to_monitor({
        type: "view",
        view: {
          key: `preview:${event.path}`,
          component: PreviewView,
          props: { image_path: event.path, name: event.name },
        },
      });
      return;
    }

    case "confirm": {
      if (state?.type === "confirm") return;
      state = { type: "confirm" };
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
            // El canal devuelve la pantalla a idle (con su estática) y
            // recién después deja que la sección resetee su estado.
            on_close: () => {
              state = null;
              emit_to_monitor({ type: "view", view: null });
              event.on_close?.();
            },
          },
        },
      });
      return;
    }

    case "clear": {
      if (state?.type === "confirm") return;
      state = null;
      emit_to_monitor({ type: "view", view: null });
      return;
    }
  }
}
