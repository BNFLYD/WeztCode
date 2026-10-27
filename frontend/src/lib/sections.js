// Fuente única de verdad de las secciones de la app + tracking de la
// activa. Los canales la usan para el gating: un canal solo emite
// actualizaciones al monitor si SU sección está activa.

export const SECTIONS = [
  { id: "explorer", icon: "solar:code-2-bold", footer_label: "ARCHIVOS", monitor_label: "コード" },
  { id: "chat", icon: "mingcute:chat-4-line", footer_label: "CHAT", monitor_label: "チャット" },
  { id: "term", icon: "devicon-plain:bash", footer_label: "TERMINAL", monitor_label: "ターミナル" },
  { id: "view", icon: "streamline-plump:web", footer_label: "VISUALIZAR", monitor_label: "表示" },
  { id: "git", icon: "mynaui:git-commit", footer_label: "GIT", monitor_label: "GIT" },
  { id: "settings", icon: "hugeicons:settings-03", footer_label: "CONFIGURACION", monitor_label: "設定" },
];

let active = "explorer";
const listeners = new Set();

export function get_active_section() {
  return active;
}

export function set_active_section(id) {
  active = id;
  for (const fn of listeners) fn(id);
}

export function on_section_change(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
