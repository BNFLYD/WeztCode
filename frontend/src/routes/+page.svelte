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
  let monitor;
  let channel_active = false;

  function handle_channel_update(ch) {
    monitor?.handle_channel(ch);
  }


</script>

<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <div class="px-7 pt-5">
    <Monitor bind:this={monitor} bind:channel_active {active_section} on_section_change={(id) => (active_section = id)} />
  </div>
  <div class="flex-1 overflow-y-auto px-5">
    {#if active_section === "explorer"}
      <ExplorerSection {active_section} on_channel_update={handle_channel_update} {channel_active} />
    {:else if active_section === "chat"}
      <ChatSection />
    {:else if active_section === "git"}
      <GitSection />
    {:else if active_section === "view"}
      <ViewSection />
    {:else if active_section === "term"}
      <TerminalSection />
    {:else if active_section === "settings"}
      <SettingsSection />
    {/if}
  </div>

  <Footer {active_section} on_section_change={(id) => (active_section = id)} />
</div>
