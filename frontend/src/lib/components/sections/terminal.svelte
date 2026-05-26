<script>
  import { afterUpdate, onDestroy, onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let panes = [];
  let loading = true;
  let error = null;
  let labels = {};
  let renaming_id = null;
  let rename_value = "";
  let cursor_index = 0;
  let creating = false;
  let create_name = "";
  let create_input;
  let list_ref;
  let controller = null;

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
    if (controller) controller.abort();
    controller = new AbortController();
    const signal = controller.signal;
    loading = true;
    error = null;
    cursor_index = 0;
    try {
      const res = await fetch("/api/terminal/list", { signal });
      const json = await res.json();
      if (json.ok) {
        panes = (json.data || [])
          .map((line) => {
            const cols = line.split(/\s+/);
            return {
              tab_id: parseInt(cols[1]) || 0,
              pane_id: parseInt(cols[2]) || 0,
              title: cols.slice(5).join(" "),
              raw: line,
            };
          })
          .filter((p) => p.tab_id !== 0);
      } else {
        error = json.error;
      }
    } catch (e) {
      if (e.name !== "AbortError") error = e.message;
    }
    if (!signal.aborted) loading = false;
  }

  async function spawn_tab(name) {
    error = null;
    try {
      const res = await fetch("/api/terminal/spawn", { method: "POST" });
      const json = await res.json();
      if (json.ok) {
        if (name) {
          labels[json.data.pane_id] = name;
          save_labels();
        }
        await load_panes();
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
  }

  function create_terminal() {
    const name = create_name.trim();
    spawn_tab(name || undefined);
    creating = false;
    create_name = "";
  }

  function cancel_create() {
    creating = false;
    create_name = "";
  }

  async function kill_pane(pane_id) {
    if (pane_id === 0) return;
    error = null;
    try {
      const res = await fetch(`/api/terminal/kill?pane_id=${pane_id}`, {
        method: "POST",
      });
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
      const res = await fetch(`/api/terminal/activate?pane_id=${pane_id}`, {
        method: "POST",
      });
      const json = await res.json();
      if (!json.ok) {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
  }

  function move_cursor(delta) {
    const new_index = cursor_index + delta;
    if (new_index < 0 || new_index >= panes.length) return;
    cursor_index = new_index;
    scroll_to_cursor();
  }

  function scroll_to_cursor() {
    if (!list_ref) return;
    const child = list_ref.querySelector(`[data-index="${cursor_index}"]`);
    if (child) child.scrollIntoView({ block: "nearest" });
  }

  function activate_current() {
    const pane = panes[cursor_index];
    if (pane) activate_pane(pane.pane_id);
  }

  function handle_keydown(e) {
    if (renaming_id !== null) {
      if (e.key === "Enter") commit_rename();
      else if (e.key === "Escape") cancel_rename();
      return;
    }

    if (creating) {
      if (e.key === "Enter") { e.preventDefault(); create_terminal(); }
      else if (e.key === "Escape") { e.preventDefault(); cancel_create(); }
      return;
    }

    switch (e.key) {
      case "j":
      case "ArrowDown":
        e.preventDefault();
        move_cursor(1);
        break;
      case "k":
      case "ArrowUp":
        e.preventDefault();
        move_cursor(-1);
        break;
      case "l":
      case "Enter":
        e.preventDefault();
        activate_current();
        break;
      case "r":
      case "R":
        e.preventDefault();
        start_rename(panes[cursor_index]?.pane_id);
        break;
      case "d":
      case "D":
        e.preventDefault();
        kill_pane(panes[cursor_index]?.pane_id);
        break;
      case "a":
      case "A":
        e.preventDefault();
        creating = true;
        create_name = "";
        break;
    }
  }

  afterUpdate(() => {
    if (creating && create_input) create_input.focus();
    if (!loading && list_ref && panes.length > 0) {
      scroll_to_cursor();
    }
  });

  onDestroy(() => {
    if (controller) controller.abort();
  });
</script>

<svelte:window on:keydown={handle_keydown} />

<div class="flex flex-col gap-1 py-2 h-full">
  <div class="flex items-center gap-2 px-3 py-2 border-b border-accent-detail/20 mb-2 flex-shrink-0">
  <button
      on:click={() => { creating = true; create_name = ""; }}
      class="flex items-center gap-1 text-xs text-accent-detail hover:text-print transition-colors"
    >
      <Icon icon="lucide:plus" class="w-4 h-4" />
    </button>
    {#if creating}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print text-sm">{_>}</span>
        <input
          bind:value={create_name}
          bind:this={create_input}
          placeholder="terminal name"
          class="bg-transparent outline-none text-print font-mono text-xs flex-1 min-w-0"
          on:blur={() => { if (!create_name.trim()) creating = false; }}
        />
      </span>
    {:else}
      <span class="text-sm text-print-contrast font-bold tracking-wide flex-1">TERMINALS</span>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto min-h-0 px-1" bind:this={list_ref}>
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
        <span class="text-print/50 text-lg">0 tabs</span>
      </div>
    {:else}
      {#each panes as pane, index}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <div
          class={"px-3 py-2 rounded-lg hover:bg-accent/5 transition-colors cursor-pointer flex items-center gap-2" +
            (cursor_index === index ? " bg-accent/10" : "")}
          title={pane.raw}
          data-index={index}
          on:click={() => {
            cursor_index = index;
            activate_current();
          }}
        >
          <div class="flex-1 min-w-0">
            {#if renaming_id === pane.pane_id}
              <input
                type="text"
                bind:value={rename_value}
                use:focus_on_mount
                class="bg-back rounded px-1 py-2 text-print text-sm w-full outline-none border border-accent-detail/40"
                on:blur={commit_rename}
                on:keydown={(e) => {
                  if (e.key === "Enter") commit_rename();
                  if (e.key === "Escape") cancel_rename();
                }}
              />
            {:else}
              <div class="text-print font-lg truncate leading-tight">
                {labels[pane.pane_id] || pane.title || `Pane ${pane.pane_id}`}
              </div>
            {/if}
          </div>
          <span class="font-bold font-sm text-accent-detail shrink-0 text-xs"
            >T{pane.tab_id}</span
          >
        </div>
      {/each}
    {/if}
  </div>
</div>
