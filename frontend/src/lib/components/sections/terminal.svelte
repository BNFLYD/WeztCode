<script>
  import { afterUpdate, onDestroy, onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let panes = [];
  let loading = true;
  let error = null;
  let labels = {};
  let renaming = false;
  let rename_name = "";
  let rename_input;
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

  function rename_entry() {
    const name = rename_name.trim();
    if (name) {
      const pane = panes[cursor_index];
      if (pane) {
        labels[pane.pane_id] = name;
        save_labels();
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

  function terminal_icon(pane) {
    const name = labels[pane.pane_id] || pane.title || "";
    const lower = name.toLowerCase();
    if (lower.includes("zsh")) return "devicon-plain:terminal";
    if (lower.includes("powershell") || lower.includes("pwsh")) return "devicon-plain:windows8";
    if (lower.includes("cmd")) return "devicon-plain:windows8";
    if (lower.includes("node")) return "devicon-plain:nodejs";
    if (lower.includes("python") || lower.includes("py")) return "devicon-plain:python";
    if (lower.includes("git")) return "devicon-plain:git";
    if (lower.includes("docker")) return "devicon-plain:docker";
    if (lower.includes("rust") || lower.includes("cargo")) return "devicon-plain:rust";
    return "devicon-plain:bash";
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
        rename_name = labels[panes[cursor_index]?.pane_id] || "";
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
    if (controller) controller.abort();
  });
</script>

<svelte:window on:keydown={handle_keydown} />

<div class="flex flex-col gap-1 py-2 h-full">
  <div class="flex items-center gap-2 px-3 py-2 border-b border-accent-detail/20 mb-2 flex-shrink-0">
  <button
      on:click={() => { creating = true; create_name = ""; }}
      class="flex items-center gap-1 text-xs text-print/50 hover:text-print transition-colors"
    >
      <Icon icon="lucide:plus" class="w-4 h-4" />
    </button>
    {#if creating}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print text-sm">$_</span>
        <input
          bind:value={create_name}
          bind:this={create_input}
          placeholder="terminal name"
          class="bg-transparent outline-none text-print font-mono text-xs flex-1 min-w-0"
          on:blur={() => { if (!create_name.trim()) creating = false; }}
        />
      </span>
    {:else if renaming}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print text-sm">>_</span>
        <input
          bind:value={rename_name}
          bind:this={rename_input}
          placeholder="rename"
          class="bg-transparent outline-none text-print font-mono text-xs flex-1 min-w-0"
          on:blur={rename_entry}
        />
      </span>
    {:else}
      <span class="text-sm text-print font-bold tracking-wide flex-1">[~]#</span>
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
      {#each panes as pane, index}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <div
          class={"py-3 px-2 rounded-lg hover:bg-accent/5 transition-colors cursor-pointer flex items-center gap-2" +
            (cursor_index === index ? " bg-accent/10" : "")}
          title={pane.raw}
          data-index={index}
          on:click={() => {
            cursor_index = index;
            activate_current();
          }}
        >
          <div class="flex-1 min-w-0 flex items-center gap-2">
            <span class="text-accent-detail shrink-0">
              <Icon icon={terminal_icon(pane)} class="w-4 h-4" />
            </span>
            <div class="text-print font-lg truncate leading-tight">
              {labels[pane.pane_id] || pane.title || `Pane ${pane.pane_id}`}
            </div>
          </div>
          <span class="font-bold font-sm text-accent-detail shrink-0 text-xs"
            >T{pane.tab_id}</span
          >
        </div>
      {/each}
    {/if}
  </div>
</div>
