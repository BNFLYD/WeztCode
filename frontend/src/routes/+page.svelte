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
  let preview_image = null;
  let preview_timeout = null;
  let channel_timeout = null;

  function handle_channel(ch) {
    if (preview_timeout) clearTimeout(preview_timeout);
    preview_timeout = null;
    preview_image = null;
    if (channel_timeout) return;
    if (active_channel) return;
    is_distorting = true;
    channel_timeout = setTimeout(() => {
      active_channel = ch;
      channel_timeout = null;
      setTimeout(() => {
        is_distorting = false;
      }, 200);
    }, 300);
  }

  function handle_channel_close() {
    if (is_distorting) return;
    if (channel_timeout) clearTimeout(channel_timeout);
    channel_timeout = null;
    active_channel = null;
    is_distorting = true;
    channel_timeout = setTimeout(() => {
      is_distorting = false;
      channel_timeout = null;
    }, 300);
  }

  function handle_preview(path) {
    if (channel_timeout) return;
    if (is_distorting && active_channel?.id !== "preview") return;
    if (preview_timeout) clearTimeout(preview_timeout);
    preview_timeout = null;
    preview_image = path;
    if (!path) {
      is_distorting = true;
      setTimeout(() => { is_distorting = false; }, 150);
    } else {
      is_distorting = true;
      preview_timeout = setTimeout(() => {
        is_distorting = false;
        preview_timeout = null;
      }, 80);
    }
  }
</script>

<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <div class="px-7 pt-5">
    <Monitor {active_section} channel={active_channel} {is_distorting} {preview_image} on_channel_close={handle_channel_close} on_section_change={(id) => (active_section = id)} on_preview_close={() => handle_preview(null)} />
  </div>
  <div class="flex-1 overflow-y-auto px-5">
    {#if active_section === "explorer"}
      <ExplorerSection {active_section} channel={active_channel} on_channel={handle_channel} on_channel_close={handle_channel_close} on_image_preview={handle_preview} />
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
