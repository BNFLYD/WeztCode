<script>
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let lines = [];
  let loading = true;
  let error = null;

  onMount(() => {
    load_panes();
  });

  async function load_panes() {
    loading = true;
    error = null;
    try {
      const res = await fetch("/api/terminal/list");
      const json = await res.json();
      if (json.ok) {
        lines = json.data;
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
    loading = false;
  }

  async function spawn_tab() {
    error = null;
    try {
      const res = await fetch("/api/terminal/spawn", { method: "POST" });
      const json = await res.json();
      if (json.ok) {
        await load_panes();
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
  }
</script>

<div class="flex flex-col gap-2 py-4 h-full">
  <div class="flex items-center justify-between px-3 pb-2 border-b border-accent-detail/20 mb-2 flex-shrink-0">
    <span class="text-sm text-print-contrast font-bold tracking-wide">TERMINAL TABS</span>
    <button
      on:click={spawn_tab}
      class="flex items-center gap-1 text-xs text-accent-detail hover:text-print transition-colors"
    >
      <Icon icon="lucide:plus" class="w-4 h-4" />
      <span>New</span>
    </button>
  </div>

  <div class="flex-1 overflow-y-auto min-h-0 space-y-1 px-1">
    {#if loading}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">Loading...</span>
      </div>
    {:else if error}
      <div class="flex items-center justify-center py-8">
        <span class="text-accent-err text-lg">{error}</span>
      </div>
    {:else if lines.length === 0}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">No terminals</span>
      </div>
    {:else}
      {#each lines as line, i}
        <div
          class="font-mono text-[11px] text-print/80 px-3 py-1.5 truncate hover:bg-accent/5 rounded-lg"
          title={line}
        >
          <span class="text-accent-detail/60 mr-2">#{i}</span>
          {line}
        </div>
      {/each}
    {/if}
  </div>
</div>
