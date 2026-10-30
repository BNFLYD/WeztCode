<script context="module">
  let saved_state = { path: "/", entry_name: null };
</script>

<script>
  import { afterUpdate, onDestroy } from "svelte";
  import Icon from "@iconify/svelte";
  import NfIcon from "$lib/components/ui/nf_icon.svelte";
  // Canal del explorer: la sección solo detecta eventos y dispara
  // signals; las vistas del monitor (preview + cartel Si/No) viven en
  // el canal, y las transiciones (estática) en el monitor.
  import * as explorer_channel from "$lib/components/ui/channels/explorer_channel.js";

  export let active_section = "explorer";
  export let pending_explorer_path = null;

  let current_path = "/";
  let entries = [];
  let loading = true;
  let error = null;
  let cursor_index = 0;
  let list_ref;
  let creating = false;
  let create_name = "";
  let create_input;
  let renaming = false;
  let rename_name = "";
  let rename_input;
  let clipboard = null;
  let moving = false;
  let move_name = "";
  let move_input;
  let controller = null;
  let show_projects = false;
  let switching_project = false;
  let projects = [];
  let project_loading = false;
  let pending_g = false;

  // Diálogo de confirmación abierto: bloquea el teclado de la sección.
  // El canal le gana la precedencia al preview mientras esté abierto.
  let dialog_open = false;

  // Preview automático: cursor sobre imagen/GIF → signal al canal del
  // explorer (clear para todo lo demás). La transición (estática →
  // vista) la maneja el monitor; acá no hay tiempos ni estados del
  // monitor. Al cerrarse el diálogo esto re-sincroniza el preview del
  // archivo bajo el cursor.
  $: {
    const entry = show_projects ? null : entries[cursor_index];
    if (!dialog_open) {
      if (entry && entry.entry_type === "file") {
        if (is_image(entry.name)) {
          explorer_channel.signal({ type: "preview", path: entry.path, icon: file_icon(entry.name), name: entry.name });
        } else {
          explorer_channel.signal({ type: "clear" });
        }
      } else {
        explorer_channel.signal({ type: "clear" });
      }
    }
  }

  async function load_dir(path, focus_name = null) {
    if (controller) controller.abort();
    controller = new AbortController();
    const signal = controller.signal;
    loading = true;
    error = null;
    cursor_index = 0;
    try {
      const res = await fetch(`/api/fs/ls?path=${encodeURIComponent(path)}`, { signal });
      const json = await res.json();
      if (json.ok) {
        entries = json.data.files;
        current_path = json.data.path;
        if (focus_name) {
          const idx = entries.findIndex(e => e.name === focus_name);
          if (idx !== -1) cursor_index = idx;
        }
      } else {
        error = json.error;
      }
    } catch (e) {
      if (e.name !== "AbortError") {
        error = e.message;
      }
    }
    if (!signal.aborted) loading = false;
  }

  // Iconos de archivo/directorio: nombre de Nerd Font (Iconify nf-*).
  // El glifo sale de nerd_glyphs.js; se renderiza como texto.
  function file_icon(name) {
    if (name.endsWith(".rs")) return "nf-dev-rust";
    if (name.endsWith(".js") || name.endsWith(".ts")) return "nf-dev-javascript";
    if (name.endsWith(".svelte")) return "nf-dev-svelte";
    if (name.endsWith(".lua")) return "nf-dev-lua";
    if (name.endsWith(".css") || name.endsWith(".html")) return "nf-dev-html5";
    if (name.endsWith(".md")) return "nf-dev-markdown";
    if (name.endsWith(".json")) return "nf-dev-json";
    if (name.endsWith(".toml")) return "nf-md-cog";
    if (name.endsWith(".py")) return "nf-dev-python";
    if (name.endsWith(".png") || name.endsWith(".jpg") || name.endsWith(".jpeg") || name.endsWith(".webp") || name.endsWith(".svg") || name.endsWith(".bmp") || name.endsWith(".ico")) return "nf-md-image";
    if (name.endsWith(".gif")) return "nf-md-movie";
    return "nf-fa-file";
  }

  function dir_icon() {
    return "nf-cod-folder";
  }

  // Extensiones con preview en el monitor (imágenes y GIFs).
  const IMAGE_EXTS = [".png", ".jpg", ".jpeg", ".webp", ".svg", ".bmp", ".ico", ".gif"];

  function is_image(name) {
    const lower = name.toLowerCase();
    return IMAGE_EXTS.some(ext => lower.endsWith(ext));
  }

  async function open_file(path) {
    const res = await fetch('/api/terminal/active-pane');
    const json = await res.json();
    if (json.ok && json.data.pane_id !== 0) {
      await fetch('/api/terminal/ensure-main', { method: 'POST' }); 
    }
    fetch(`/api/editor/open?path=${encodeURIComponent(path)}`);
  }

  function open_dir(name) {
    load_dir(current_path === "/" ? name : `${current_path}/${name}`);
  }

  async function create_entry() {
    const name = create_name.trim();
    if (!name) { creating = false; return; }
    const target = current_path === "/"
      ? name
      : `${current_path}/${name}`;
    const res = await fetch(`/api/fs/create?path=${encodeURIComponent(target)}`);
    const json = await res.json();
    creating = false;
    create_name = "";
    if (json.ok) {
      await load_dir(current_path);
    } else {
      error = json.error;
    }
  }

  async function rename_entry() {
    const name = rename_name.trim();
    if (!name) { renaming = false; return; }
    const entry = entries[cursor_index];
    if (!entry) { renaming = false; return; }
    const res = await fetch(`/api/fs/rename?path=${encodeURIComponent(entry.path)}&name=${encodeURIComponent(name)}`);
    const json = await res.json();
    renaming = false;
    rename_name = "";
    if (json.ok) {
      await load_dir(current_path);
    } else {
      error = json.error;
    }
  }

  function cut_entry() {
    const entry = entries[cursor_index];
    if (!entry) return;
    clipboard = { entry, operation: "cut" };
  }

  async function paste_entry() {
    if (!clipboard || clipboard.operation !== "cut") return;
    const name = clipboard.entry.name;
    const target = current_path === "/" ? name : `${current_path}/${name}`;
    const res = await fetch(`/api/fs/move?path=${encodeURIComponent(clipboard.entry.path)}&dest=${encodeURIComponent(target)}`);
    const json = await res.json();
    if (json.ok) {
      clipboard = null;
      await load_dir(current_path);
    } else {
      error = json.error;
    }
  }

  async function move_entry() {
    const name = move_name.trim();
    if (!name) { moving = false; return; }
    const entry = entries[cursor_index];
    if (!entry) { moving = false; return; }
    const target = current_path === "/" ? name : `${current_path}/${name}`;
    const res = await fetch(`/api/fs/move?path=${encodeURIComponent(entry.path)}&dest=${encodeURIComponent(target)}`);
    const json = await res.json();
    moving = false;
    move_name = "";
    if (json.ok) {
      await load_dir(current_path);
    } else {
      error = json.error;
    }
  }

  function confirm_delete() {
    const entry = entries[cursor_index];
    if (!entry) return;
    dialog_open = true;
    explorer_channel.signal({
      type: "confirm",
      icon: entry.entry_type === "dir" ? dir_icon() : file_icon(entry.name),
      name: entry.name,
      on_confirm: async () => {
        const res = await fetch(`/api/fs/delete?path=${encodeURIComponent(entry.path)}`);
        const json = await res.json();
        if (json.ok) {
          await load_dir(current_path);
        } else {
          error = json.error;
        }
      },
      on_cancel: () => {},
      on_close: () => {
        dialog_open = false;
      }
    });
  }

  function go_up() {
    if (current_path === "/") return;
    const parts = current_path.split("/").filter(Boolean);
    const focus_name = parts.pop();
    const parent = parts.length === 0 ? "/" : "/" + parts.join("/");
    load_dir(parent, focus_name);
  }

  async function load_projects() {
    show_projects = true;
    project_loading = true;
    error = null;
    cursor_index = 0;
    try {
      const res = await fetch("/api/projects/list");
      const json = await res.json();
      if (json.ok) {
        projects = json.data || [];
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
    project_loading = false;
  }

  function exit_projects() {
    show_projects = false;
    projects = [];
  }

  async function select_project(path) {
    if (switching_project) return;
    switching_project = true;
    try {
      const res = await fetch(`/api/projects/switch?path=${encodeURIComponent(path)}`);
      const json = await res.json();
      if (json.ok) {
        show_projects = false;
        projects = [];
        // El switch terminó (el backend ya corre con la sesión del proyecto
        // NUEVO): avisarle al resto de la app. El chat escucha este evento y
        // re-sincroniza agente/historial — sin esto, un init que corrió durante
        // el switch quedaba con el estado del proyecto VIEJO hasta recargar.
        window.dispatchEvent(new CustomEvent("weztcode:project-switched"));
        await load_dir("/");
      } else {
        error = json.error;
      }
    } finally {
      switching_project = false;
    }
  }

  async function create_project_entry() {
    const name = create_name.trim();
    if (!name) { creating = false; return; }
    const res = await fetch("/api/projects/add", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ path: name }),
    });
    const json = await res.json();
    creating = false;
    create_name = "";
    if (json.ok) {
      projects = json.data || [];
    } else {
      error = json.error;
    }
  }

  async function delete_project_entry(path) {
    const res = await fetch(`/api/projects/delete?path=${encodeURIComponent(path)}`);
    const json = await res.json();
    if (json.ok) {
      projects = json.data || [];
    } else {
      error = json.error;
    }
  }

  function move_cursor(delta) {
    const len = show_projects ? projects.length : entries.length;
    const new_index = cursor_index + delta;
    if (new_index < 0 || new_index >= len) return;
    cursor_index = new_index;
    scroll_to_cursor();
  }

  function scroll_to_cursor() {
    if (!list_ref) return;
    const child = list_ref.querySelector(`[data-index="${cursor_index}"]`);
    if (child) child.scrollIntoView({ block: "nearest" });
  }

  afterUpdate(() => {
    if (creating && create_input) create_input.focus();
    else if (renaming && rename_input) rename_input.focus();
    else if (moving && move_input) move_input.focus();
    else if (!loading && list_ref && entries.length > 0) {
      scroll_to_cursor();
    }
  });

  onDestroy(() => {
    saved_state = {
      path: current_path,
      entry_name: entries[cursor_index]?.name ?? null
    };
    if (controller) controller.abort();
    show_projects = false;
    projects = [];
    pending_g = false;
  });

  function activate_current() {
    const entry = entries[cursor_index];
    if (!entry) return;
    if (entry.entry_type === "dir") open_dir(entry.name);
    else open_file(entry.path);
  }

  function handle_keydown(e) {
    if (!document.hasFocus()) return;
    if (active_section !== "explorer") return;

    if (pending_g) {
      if (e.key === "h" || e.key === "H") {
        e.preventDefault();
        pending_g = false;
        load_projects();
        return;
      }
      pending_g = false;
    }

    if (creating) {
      if (e.key === "Enter") {
        e.preventDefault();
        if (show_projects) create_project_entry();
        else create_entry();
      } else if (e.key === "Escape") {
        e.preventDefault();
        creating = false;
        create_name = "";
      }
      return;
    }

    if (renaming) {
      if (e.key === "Enter") {
        e.preventDefault();
        rename_entry();
      } else if (e.key === "Escape") {
        e.preventDefault();
        renaming = false;
        rename_name = "";
      }
      return;
    }

    if (moving) {
      if (e.key === "Enter") {
        e.preventDefault();
        move_entry();
      } else if (e.key === "Escape") {
        e.preventDefault();
        moving = false;
        move_name = "";
      }
      return;
    }

    if (dialog_open) {
      if (["h","l","j","k","ArrowLeft","ArrowRight","ArrowUp","ArrowDown","d","D","a","A","r","R","x","X","p","P","m","M","Enter"," ","Escape"].includes(e.key)) {
        e.preventDefault();
      }
      return;
    }

    if (show_projects) {
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
        case "ArrowRight":
        case "Enter":
        case " ":
          e.preventDefault();
          const proj = projects[cursor_index];
          if (proj) select_project(proj.path);
          break;
        case "h":
        case "ArrowLeft":
        case "Escape":
          e.preventDefault();
          exit_projects();
          break;
        case "a":
        case "A":
          e.preventDefault();
          creating = true;
          create_name = "";
          break;
        case "d":
        case "D":
          e.preventDefault();
          const entry = projects[cursor_index];
          if (!entry) break;
          dialog_open = true;
          explorer_channel.signal({
            type: "confirm",
            icon: "nf-cod-folder_opened",
            name: entry.name,
            on_confirm: async () => {
              await delete_project_entry(entry.path);
            },
            on_cancel: () => {},
            on_close: () => {
              dialog_open = false;
            }
          });
          break;
      }
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
      case "ArrowRight":
        e.preventDefault();
        activate_current();
        break;
      case "h":
      case "ArrowLeft":
        e.preventDefault();
        go_up();
        break;
      case "r":
      case "R":
        e.preventDefault();
        if (!entries[cursor_index]) break;
        renaming = true;
        rename_name = entries[cursor_index].name;
        break;
      case "x":
      case "X":
        e.preventDefault();
        cut_entry();
        break;
      case "p":
      case "P":
        e.preventDefault();
        paste_entry();
        break;
      case "m":
      case "M":
        e.preventDefault();
        if (!entries[cursor_index]) break;
        moving = true;
        move_name = entries[cursor_index].name;
        break;
      case "d":
      case "D":
        e.preventDefault();
        confirm_delete();
        break;
      case "a":
      case "A":
        e.preventDefault();
        creating = true;
        create_name = "";
        break;
      case "g":
      case "G":
        e.preventDefault();
        pending_g = true;
        break;
      case "Enter":
        e.preventDefault();
        activate_current();
        break;
      case " ":
        e.preventDefault();
        activate_current();
        break;
    }
  }

  if (pending_explorer_path) {
    load_dir(pending_explorer_path);
  } else {
    load_dir(saved_state.path, saved_state.entry_name);
  }
</script>

<svelte:window on:keydown={handle_keydown} />

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0">
    <button on:click={show_projects ? exit_projects : go_up} class="hover:text-print transition-colors">
      <NfIcon icon={show_projects ? "nf-cod-arrow_left" : "nf-cod-folder"} />
    </button>
    {#if show_projects}
      {#if creating}
        <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
          <span class="text-print-contrast">path:</span>
          <input
            bind:value={create_name}
            bind:this={create_input}
            placeholder="/home/user/projects"
            class="bg-transparent outline-none text-print font-mono flex-1 min-w-0"
            on:blur={() => { if (!create_name.trim()) creating = false; }}
          />
        </span>
      {:else}
        <span class="font-mono truncate-start text-print-contrast">proyectos</span>
      {/if}
    {:else if creating}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print-contrast">add:</span>
        <input
          bind:value={create_name}
          bind:this={create_input}
          placeholder={current_path}
          class="bg-transparent outline-none text-print font-mono flex-1 min-w-0"
          on:blur={() => { if (!create_name.trim()) creating = false; }}
        />
      </span>
    {:else if renaming}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print-contrast">upd:</span>
        <input
          bind:value={rename_name}
          bind:this={rename_input}
          placeholder={entries[cursor_index]?.name ?? ""}
          class="bg-transparent outline-none text-print font-mono flex-1 min-w-0"
          on:blur={() => { if (!rename_name.trim()) renaming = false; }}
        />
      </span>
    {:else if moving}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1">
        <span class="text-print-contrast">mv:</span>
        <input
          bind:value={move_name}
          bind:this={move_input}
          placeholder={entries[cursor_index]?.name ?? ""}
          class="bg-transparent outline-none text-print font-mono flex-1 min-w-0"
          on:blur={() => { if (!move_name.trim()) moving = false; }}
        />
      </span>
    {:else}
      <span class="font-mono truncate-start">{current_path}</span>
    {/if}
  </div>

  <div class="flex-1 overflow-y-auto min-h-0" bind:this={list_ref}>
    {#if show_projects}
      {#if project_loading}
        <div class="flex items-center justify-center py-8">
          <span class="text-print/50 text-lg">Loading...</span>
        </div>
      {:else if error}
        <div class="flex items-center justify-center py-8">
          <span class="text-accent-err text-lg">{error}</span>
        </div>
      {:else if projects.length === 0 && !creating}
        <div class="flex items-center justify-center py-8">
          <span class="text-print/50 text-lg">No hay proyectos</span>
        </div>
      {:else}
        {#if switching_project}
          <div class="flex items-center gap-2 px-3 py-2 text-print/60 text-sm font-mono border-b border-accent/10">
            <Icon icon="svg-spinners:bars-scale-fade" class="w-4 h-4" />
            Cambiando de proyecto…
          </div>
        {/if}
        {#each projects as proj, index (proj.path)}
          <!-- svelte-ignore a11y-click-events-have-key-events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->

<div class={"flex items-center gap-2 px-3 py-2.5 rounded-lg transition-colors text-lg group cursor-pointer "
    + (cursor_index === index ? "bg-accent/10" : "hover:bg-accent/5")}
            data-index={index}
            on:click={() => {
              cursor_index = index;
              select_project(proj.path);
            }}
          >
            <span class="text-accent-detail">
              <NfIcon icon="nf-cod-folder_opened" />
            </span>
            <span class="text-print font-medium flex-shrink-0">{proj.name}</span>
            <span class="text-print/30 text-xs truncate ml-2">{proj.path}</span>
          </div>
        {/each}
      {/if}
    {:else if loading}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">Loading...</span>
      </div>
    {:else if error}
      <div class="flex items-center justify-center py-8">
        <span class="text-accent-err text-lg">{error}</span>
      </div>
    {:else if entries.length === 0}
      <div class="flex items-center justify-center py-8">
        <span class="text-print/50 text-lg">Empty directory</span>
      </div>
    {:else}
      {#each entries as entry, index (entry.path)}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class={"flex items-center gap-2 px-3 py-1.5 rounded-lg transition-colors text-lg group cursor-pointer"
            + (cursor_index === index ? " bg-accent/10" : "hover:bg-accent/5")
            + (clipboard?.entry.path === entry.path ? " opacity-40" : "")}
          data-index={index}
        on:click={() => {
          cursor_index = index;
          if (entry.entry_type === "dir") open_dir(entry.name);
          else open_file(entry.path);
        }}
        >
          {#if entry.entry_type === "dir"}
            <span class="text-accent-detail">
              {#if clipboard?.entry.path === entry.path}
                <NfIcon icon="nf-fa-scissors" />
              {:else}
                <NfIcon icon={dir_icon()} />
              {/if}
            </span>
            <span class="text-print font-medium">{entry.name}</span>
          {:else}
            <span class="text-accent-detail">
              {#if clipboard?.entry.path === entry.path}
                <NfIcon icon="nf-fa-scissors" />
              {:else}
                <NfIcon icon={file_icon(entry.name)} />
              {/if}
            </span>
            <span class="text-print/70 flex-1 truncate">{entry.name}</span>
            {#if entry.size}
              <span class="text-print/30 text-xs">{Math.round(entry.size / 1024)}KB</span>
            {/if}
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>
