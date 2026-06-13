<script context="module">
  let saved_state = { pane_id: null };
</script>

<script>
  import { afterUpdate, onDestroy, onMount } from "svelte";
  import Icon from "@iconify/svelte";

  export let active_section = "term";

  let panes = [];
  let metadata = {};
  let loading = true;
  let error = null;
  let renaming = false;
  let rename_name = "";
  let rename_input;
  let cursor_index = 0;
  let creating = false;
  let create_name = "";
  let create_input;
  let list_ref;
  let controller = null;
  let load_timeout = null;

  onMount(() => {
    load_panes();
  });

  async function rename_entry() {
    const input = rename_name.trim();
    let name = input;
    let icon = null;
    const slash_idx = input.indexOf("/");
    if (slash_idx !== -1) {
      name = input.slice(0, slash_idx).trim();
      icon = input.slice(slash_idx + 1).trim();
    }
    const pane = panes[cursor_index];
    if (pane) {
      const res = await fetch("/api/terminal/metadata/set", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          pane_id: pane.pane_id,
          name: name || null,
          icon: icon || null,
        }),
      });
      const json = await res.json();
      if (json.ok) {
        metadata = { ...metadata, [pane.pane_id]: { name: name || null, icon: icon || null } };
      } else {
        error = json.error;
      }
    }
    renaming = false;
    rename_name = "";
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
        panes = (json.data.panes || []).filter((p) => p.tab_id !== 0);
        metadata = json.data.metadata || {};
        if (saved_state.pane_id !== null) {
          const idx = panes.findIndex(p => p.pane_id === saved_state.pane_id);
          if (idx !== -1) cursor_index = idx;
          saved_state.pane_id = null;
        }
      } else {
        error = json.error;
      }
    } catch (e) {
      if (e.name !== "AbortError") error = e.message;
    }
    if (!signal.aborted) loading = false;
  }

  async function spawn_tab(name, icon, program) {
    error = null;
    try {
      const res = await fetch("/api/terminal/spawn", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name, icon, program }),
      });
      const json = await res.json();
      if (json.ok) {
        if (json.data.name || json.data.icon) {
          metadata = { ...metadata, [json.data.pane_id]: { name: json.data.name, icon: json.data.icon } };
        }
        schedule_load();
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
  }

  async function create_terminal() {
    const input = create_name.trim();

    if (input.startsWith("dterm:")) {
      const dterm_name = input.slice(6).trim();
      if (!dterm_name) {
        error = "Debe especificar un nombre: dterm:Yazi";
        creating = false;
        create_name = "";
        return;
      }
      try {
        const res = await fetch("/api/terminal/default-terms");
        const json = await res.json();
        if (json.ok) {
          const found = json.data.find(t => t.name === dterm_name);
          if (found) {
            await spawn_tab(found.name, found.icon, found.program);
          } else {
            error = `Terminal por defecto inválida: "${dterm_name}"`;
          }
        } else {
          error = json.error;
        }
      } catch (e) {
        error = e.message;
      }
      creating = false;
      create_name = "";
      return;
    }

    let name = input;
    let icon = null;
    const slash_idx = input.indexOf("/");
    if (slash_idx !== -1) {
      name = input.slice(0, slash_idx).trim();
      icon = input.slice(slash_idx + 1).trim();
    }
    await spawn_tab(name || undefined, icon || undefined, undefined);
    creating = false;
    create_name = "";
  }

  function cancel_create() {
    creating = false;
    create_name = "";
  }

  function terminal_icon(pane) {
    const meta = metadata[pane.pane_id];
    if (meta?.icon) return meta.icon;
    return "file-icons:terminal";
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
        await fetch(`/api/terminal/metadata/delete?pane_id=${pane_id}`);
        const { [pane_id]: _, ...rest } = metadata;
        metadata = rest;
        schedule_load();
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

  function schedule_load() {
    if (load_timeout) clearTimeout(load_timeout);
    load_timeout = setTimeout(() => {
      load_panes();
      load_timeout = null;
    }, 300);
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
    if (!document.hasFocus()) return;
    if (active_section !== "term") return;
    if (renaming) {
      if (e.key === "Enter") { e.preventDefault(); rename_entry(); }
      else if (e.key === "Escape") { e.preventDefault(); renaming = false; rename_name = ""; }
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
        renaming = true;
        rename_name = metadata[panes[cursor_index]?.pane_id]?.name || "";
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
    else if (renaming && rename_input) rename_input.focus();
    if (!loading && list_ref && panes.length > 0) {
      scroll_to_cursor();
    }
  });

  onDestroy(() => {
    const pane = panes[cursor_index];
    if (pane) saved_state.pane_id = pane.pane_id;
    if (controller) controller.abort();
    if (load_timeout) clearTimeout(load_timeout);
  });
</script>

<svelte:window on:keydown={handle_keydown} />

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0">
  <button
      on:click={() => { creating = true; create_name = ""; }}
      class="flex items-center gap-1 text-xs text-print/50 hover:text-print transition-colors"
    >
      <Icon icon="tabler:plus-filled" class="w-4 h-4" />
    </button>
    {#if creating}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print-contrast text-sm font-semibold">$_</span>
        <input
          bind:value={create_name}
          bind:this={create_input}
          placeholder="new.sh"
          class="bg-transparent outline-none text-print font-mono text-sm flex-1 min-w-0"
          on:blur={() => { if (!create_name.trim()) creating = false; }}
        />
      </span>
    {:else if renaming}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print-contrast text-sm font-semibold">>_</span>
        <input
          bind:value={rename_name}
          bind:this={rename_input}
          placeholder="rename.sh"
          class="bg-transparent outline-none text-print font-mono text-sm flex-1 min-w-0"
          on:blur={() => { if (!rename_name.trim()) renaming = false; }}
        />
      </span>
    {:else}
      <span class="text-sm text-print/50 font-semibold tracking-wide flex-1">[~]#</span>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto min-h-0" bind:this={list_ref}>
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
      {#each panes as pane, index (pane.pane_id)}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <div
          class={"flex items-center gap-2 px-3 py-2.5 rounded-lg transition-colors hover:bg-accent/5 cursor-pointer" +
            (cursor_index === index ? "bg-accent/10" : "")}
          title={pane.title || `Pane ${pane.pane_id}`}
          data-index={index}
          on:click={() => {
            cursor_index = index;
            activate_current();
          }}
        >
          <div class="flex-1 min-w-0 flex items-center gap-2">
            <span class="text-print shrink-0">
              <Icon icon={terminal_icon(pane)} class="w-4 h-4" />
            </span>
            <div class="text-print text-lg truncate leading-tight">
              {metadata[pane.pane_id]?.name || pane.title || `Pane ${pane.pane_id}`}
            </div>
          </div>
          <span class="font-bold text-xs text-accent-detail shrink-0"
            >T{pane.tab_id}</span
          >
        </div>
      {/each}
    {/if}
  </div>
</div>
