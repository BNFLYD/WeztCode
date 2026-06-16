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

  let active_section = "explorer";
  let active_channel = null;
  let is_distorting = false;
  let pending_explorer_path = null;

  function navigate_to_explorer(path) {
    pending_explorer_path = path;
    active_section = "explorer";
  }

  function handle_section_change(id) {
    if (id !== "explorer") pending_explorer_path = null;
    active_section = id;
  }
</script>

<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <div class="px-7 pt-5">
    <Monitor {active_section} bind:active_channel bind:is_distorting on_section_change={handle_section_change} />
  </div>
  <div class="flex-1 overflow-y-auto px-5">
    {#if active_section === "explorer"}
      <ExplorerSection {active_section} bind:active_channel bind:is_distorting {pending_explorer_path} />
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
