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

  function handle_channel(ch) {
    if (is_distorting) return;
    is_distorting = true;
    setTimeout(() => {
      active_channel = ch;
      is_distorting = false;
    }, 300);
  }

  function handle_channel_close() {
    if (is_distorting) return;
    is_distorting = true;
    setTimeout(() => {
      active_channel = null;
      is_distorting = false;
    }, 300);
  }
</script>

<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <div class="px-7 pt-5">
    <Monitor {active_section} channel={active_channel} {is_distorting} on_channel_close={handle_channel_close} on_section_change={(id) => (active_section = id)}  />
  </div>
  <div class="flex-1 overflow-y-auto px-5">
    {#if active_section === "explorer"}
      <ExplorerSection {active_section} channel={active_channel} on_channel={handle_channel} on_channel_close={handle_channel_close} />
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
