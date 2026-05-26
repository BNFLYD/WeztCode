<script>
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let panes = [];
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
        panes = (json.data || [])
          .map(line => {
            const cols = line.split(/\s+/);
            return {
              tab_id: parseInt(cols[1]) || 0,
              pane_id: parseInt(cols[2]) || 0,
              raw: line
            };
          })
          .filter(p => p.tab_id !== 0);
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

  async function kill_pane(pane_id) {
    error = null;
    try {
      const res = await fetch(`/api/terminal/kill?pane_id=${pane_id}`, { method: "POST" });
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

  async function activate_pane(pane_id) {
    error = null;
    try {
      const res = await fetch(`/api/terminal/activate?pane_id=${pane_id}`, { method: "POST" });
      const json = await res.json();
      if (!json.ok) {
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
    {:else if panes.length === 0}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">No terminals</span>
      </div>
    {:else}
      {#each panes as pane}
        <div
          class="font-mono text-[11px] text-print/80 px-3 py-1.5 hover:bg-accent/5 rounded-lg flex items-center gap-3 group"
          title={pane.raw}
        >
          <span class="font-bold text-accent-detail shrink-0">T{pane.tab_id}:P{pane.pane_id}</span>
          <span class="text-print/40 truncate flex-1">{pane.raw}</span>

          <button
            on:click={() => activate_pane(pane.pane_id)}
            class="opacity-0 group-hover:opacity-100 text-accent-detail/60 hover:text-accent-detail transition-all shrink-0"
            aria-label="Activate"
          >
            <Icon icon="lucide:play" class="w-3.5 h-3.5" />
          </button>

          <button
            on:click={() => kill_pane(pane.pane_id)}
            class="opacity-0 group-hover:opacity-100 text-accent-err/60 hover:text-accent-err transition-all shrink-0"
            aria-label="Kill"
          >
            <Icon icon="lucide:x" class="w-3.5 h-3.5" />
          </button>
        </div>
      {/each}
    {/if}
  </div>
</div>
