// Puente canales → monitor. Los canales emiten eventos de actualización
// ({ type: "view", view } | { type: "section", id }); el monitor es el
// único listener y dueño de las transiciones (estática → vista). Las
// secciones NUNCA hablan con el monitor: hablan con su canal.
//
// El bus es LA FRONTERA REACTIVA: los listeners corren dentro de
// untrack() para que ningún $state del monitor (current/window_active,
// leídos por apply_view) quede registrado como dependencia del effect
// o del `$:` que disparó la signal. Sin esto, el effect del chat leía
// `current` y lo re-escribía (live refresh) en la misma corrida → se
// auto-invalidaba infinitamente → effect_update_depth_exceeded → el
// flush moría a mitad de cola y la app entera dejaba de responder
// (ni Tab, ni cambio de sección).

import { untrack } from "svelte";

const listeners = new Set();

export function on_monitor_event(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function emit_to_monitor(event) {
  for (const fn of listeners) untrack(() => fn(event));
}
