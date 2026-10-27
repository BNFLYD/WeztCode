<script>
  import {
    ExplorerSection,
    ChatSection,
    GitSection,
    ViewSection,
    TerminalSection,
    SettingsSection,
  } from "$lib/components/sections";
  import { Footer, Monitor } from "$lib/components/ui";
  import { set_active_section } from "$lib/sections";
  import { emit_to_monitor } from "$lib/components/ui/monitor_bus";

  // +page ya no posee el canal del monitor: solo orquesta secciones.
  // El contrato es: sección → signal → canal → update al monitor, y el
  // monitor (dueño de is_distorting) hace estática → vista en cada
  // cambio de estado, con timing fijo. Los canales viven SOLO dentro
  // de su sección (gating por lib/sections.js en cada canal).

  let active_section = "explorer";
  let pending_explorer_path = null;

  function handle_section_change(id) {
    if (id === active_section) return;
    if (id !== "explorer") pending_explorer_path = null;
    // Primero cae el gating (los canales resetean su estado al salir
    // su sección), después el monitor "cambia de canal": UNA ventana de
    // estática que coalescea con lo que señalice la sección entrante.
    set_active_section(id);
    emit_to_monitor({ type: "section", id });
    active_section = id;
  }

  function navigate_to_explorer(path) {
    pending_explorer_path = path;
    handle_section_change("explorer");
  }
</script>

<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <div class="px-7 pt-5">
    <Monitor />
  </div>
  <div class="flex-1 overflow-y-auto px-5">
    {#if active_section === "explorer"}
      <ExplorerSection {active_section} {pending_explorer_path} />
    {:else if active_section === "chat"}
      <ChatSection />
    {:else if active_section === "git"}
      <GitSection />
    {:else if active_section === "view"}
      <ViewSection />
    {:else if active_section === "term"}
      <TerminalSection {active_section} />
    {:else if active_section === "settings"}
      <SettingsSection on_navigate_to_explorer={navigate_to_explorer} />
    {/if}
  </div>

  <Footer {active_section} on_section_change={handle_section_change} />
</div>
