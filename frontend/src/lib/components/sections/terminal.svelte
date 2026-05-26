<script>
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let panes = [];
  let loading = true;
  let error = null;
  let labels = {};
  let renaming_id = null;
  let rename_value = "";

  onMount(() => {
    try {
      const saved = localStorage.getItem("terminal_labels");
      if (saved) labels = JSON.parse(saved);
    } catch {}
    load_panes();
  });

  function save_labels() {
    try {
      localStorage.setItem("terminal_labels", JSON.stringify(labels));
    } catch {}
  }

  function focus_on_mount(node) {
    node.focus();
    node.select();
  }

  function start_rename(pane_id) {
    renaming_id = pane_id;
    rename_value = labels[pane_id] || "";
  }

  function commit_rename() {
    if (renaming_id !== null) {
      const v = rename_value.trim();
      if (v) {
        labels[renaming_id] = v;
      } else {
        delete labels[renaming_id];
      }
      save_labels();
      renaming_id = null;
    }
  }

  function cancel_rename() {
    renaming_id = null;
  }

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
              title: cols.slice(5).join(" "),
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
    if (pane_id === 0) return;
    error = null;
    try {
      const res = await fetch(`/api/terminal/kill?pane_id=${pane_id}`, { method: "POST" });
      const json = await res.json();
      if (json.ok) {
        delete labels[pane_id];
        save_labels();
        await load_panes();
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
  }

  async function activate_pane(pane_id) {
    if (pane_id === 0) return;
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
          class="font-lg text-print/80 px-3 py-1.5 hover:bg-accent/5 rounded-lg flex items-center gap-3 group"
          title={pane.raw}
        >
          <div class="flex-1 min-w-0">
            {#if renaming_id === pane.pane_id}
              <input
                type="text"
                bind:value={rename_value}
                use:focus_on_mount
                class="bg-back rounded px-1 py-0.5 text-print text-xs w-full outline-none border border-accent-detail/40"
                on:blur={commit_rename}
                on:keydown={(e) => {
                  if (e.key === "Enter") commit_rename();
                  if (e.key === "Escape") cancel_rename();
                }}
              />
            {:else}
              <div class="text-print font-medium truncate leading-tight">
                {labels[pane.pane_id] || pane.title || `Pane ${pane.pane_id}`}
              </div>
            {/if}
          </div>

          {#if renaming_id !== pane.pane_id}
            <button
              on:click|stopPropagation={() => start_rename(pane.pane_id)}
              class="opacity-0 group-hover:opacity-100 text-print/40 hover:text-print transition-all shrink-0"
              aria-label="Rename"
            >
              <Icon icon="lucide:pencil" class="w-3.5 h-3.5" />
            </button>
          {/if}

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
          <span class="font-bold text-accent-detail shrink-0">T{pane.tab_id}</span>
        </div>
      {/each}
    {/if}
  </div>
</div>
