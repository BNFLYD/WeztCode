<script>
  import {
    ExplorerSection,
    ChatSection,
    GitSection,
    ViewSection,
    TerminalSection,
    SettingsSection,
  } from "$lib/components/sections";
  import SidebarFooter from "$lib/components/SidebarFooter.svelte";

  let activeSection = "explorer";
</script>

<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <div class="flex-1 overflow-y-auto px-5">
    {#if activeSection === "explorer"}
      <ExplorerSection />
    {:else if activeSection === "chat"}
      <ChatSection />
    {:else if activeSection === "git"}
      <GitSection />
    {:else if activeSection === "view"}
      <ViewSection />
    {:else if activeSection === "term"}
      <TerminalSection />
    {:else if activeSection === "settings"}
      <SettingsSection />
    {/if}
  </div>

  <SidebarFooter {activeSection} onSectionChange={(id) => (activeSection = id)} />
</div>
