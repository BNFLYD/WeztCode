<script>
  import { afterUpdate, onDestroy } from "svelte";
  import Icon from "@iconify/svelte";
  import ExplorerChannel from "$lib/components/ui/channels/explorer_channel.svelte";

  export let active_section = "explorer";
  export let active_channel = null;
  export let is_distorting = false;

  let current_path = "/";
  let entries = [];
  let loading = true;
  let error = null;
  let cursor_index = 0;
  let list_ref;
  let window_has_focus = true;
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

  let channel_obj = null;
  let channel_timeout = null;
  let preview_data = null;
  let preview_timeout = null;

  $: channel_active = !!channel_obj;

  $: if (channel_obj) {
    active_channel = { component: ExplorerChannel, props: channel_obj.props };
  } else if (preview_data) {
    active_channel = {
      component: ExplorerChannel,
      props: {
        mode: "preview",
        image_path: preview_data.image_path,
        icon: preview_data.icon,
        name: preview_data.name,
        on_close: handle_preview_close
      }
    };
  } else {
    active_channel = null;
  }

  $: {
    const entry = entries[cursor_index];
    if (entry && entry.entry_type === 'file') {
      const icon = file_icon(entry.name);
      if (icon === "garden:file-image-fill-12" || icon === "fluent:gif-16-filled") {
        handle_channel({ id: 'preview', props: { path: entry.path, icon, name: entry.name } });
      } else {
        handle_channel(null);
      }
    } else {
      handle_channel(null);
    }
  }

  function handle_channel(ch) {
    if (!ch) {
      preview_data = null;
      return;
    }
    if (ch.id === 'preview') {
      if (preview_timeout) clearTimeout(preview_timeout);
      preview_data = { image_path: ch.props.path, icon: ch.props.icon, name: ch.props.name };
      is_distorting = true;
      preview_timeout = setTimeout(() => {
        is_distorting = false;
        preview_timeout = null;
      }, 200);
      return;
    }
    preview_data = null;
    if (channel_timeout) return;
    if (channel_obj) return;
    is_distorting = true;
    channel_timeout = setTimeout(() => {
      channel_obj = { props: ch.props };
      channel_timeout = null;
      setTimeout(() => {
        is_distorting = false;
      }, 200);
    }, 300);
  }

  function handle_channel_close() {
    preview_data = null;
    if (channel_timeout) clearTimeout(channel_timeout);
    channel_timeout = null;
    channel_obj = null;
    is_distorting = true;
    channel_timeout = setTimeout(() => {
      is_distorting = false;
      channel_timeout = null;
    }, 300);
  }

  function handle_preview_close() {
    preview_data = null;
    is_distorting = false;
    if (preview_timeout) clearTimeout(preview_timeout);
    preview_timeout = null;
  }

  async function load_dir(path) {
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

  function file_icon(name) {
    if (name.endsWith(".rs")) return "devicon-plain:rust";
    if (name.endsWith(".js") || name.endsWith(".ts")) return "devicon-plain:javascript";
    if (name.endsWith(".svelte")) return "devicon-plain:svelte";
    if (name.endsWith(".lua")) return "devicon-plain:lua";
    if (name.endsWith(".css") || name.endsWith(".html")) return "devicon-plain:html5";
    if (name.endsWith(".md")) return "devicon-plain:markdown";
    if (name.endsWith(".json")) return "devicon-plain:json";
    if (name.endsWith(".toml")) return "devicon-plain:toml";
    if (name.endsWith(".py")) return "devicon-plain:python";
    if (name.endsWith(".png") || name.endsWith(".jpg") || name.endsWith(".jpeg") || name.endsWith(".webp") || name.endsWith(".svg") || name.endsWith(".bmp") || name.endsWith(".ico")) return "garden:file-image-fill-12";
    if (name.endsWith(".gif")) return "fluent:gif-16-filled";
    return "tabler:file-filled";
  }

  function dir_icon() {
    return "mdi:folder";
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
    handle_channel({
      id: "explorer",
      props: {
        mode: "confirm",
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
        on_close: handle_channel_close
      }
    });
  }

  function go_up() {
    if (current_path === "/") return;
    const parts = current_path.split("/").filter(Boolean);
    parts.pop();
    load_dir(parts.length === 0 ? "/" : "/" + parts.join("/"));
  }

  function move_cursor(delta) {
    const new_index = cursor_index + delta;
    if (new_index < 0 || new_index >= entries.length) return;
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
    if (controller) controller.abort();
    if (channel_timeout) {
      clearTimeout(channel_timeout);
      channel_timeout = null;
    }
    if (preview_timeout) {
      clearTimeout(preview_timeout);
      preview_timeout = null;
    }
    channel_obj = null;
    preview_data = null;
    active_channel = null;
    is_distorting = false;
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

    if (creating) {
      if (e.key === "Enter") {
        e.preventDefault();
        create_entry();
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

    if (channel_active) {
      if (["h","l","j","k","ArrowLeft","ArrowRight","ArrowUp","ArrowDown","d","D","a","A","r","R","x","X","p","P","m","M","Enter"," ","Escape"].includes(e.key)) {
        e.preventDefault();
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

  load_dir("/");
</script>

<svelte:window
  on:keydown={handle_keydown}
  on:focus={() => {
    window_has_focus = true;
    if (!loading) load_dir(current_path);
  }}
  on:blur={() => window_has_focus = false}
/>

<div class="flex flex-col gap-1 py-2 h-full relative">
  <div class="flex items-center gap-2 px-3 py-2 text-sm text-accent-detail/50 border-b border-accent-detail/20 mb-2 flex-shrink-0">
    <button on:click={go_up} class="hover:text-print transition-colors">
      <Icon icon="tabler:folder" class="w-4 h-4" />
    </button>
    {#if creating}
      <span class="font-mono truncate flex items-center gap-1 text-print">
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
      <span class="font-mono truncate flex items-center gap-1 text-print">
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
      <span class="font-mono truncate flex items-center gap-1 text-print">
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
    {#if loading}
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
        <div class={"flex items-center gap-2 px-3 py-3 rounded-lg transition-colors text-lg group cursor-pointer hover:bg-accent/5"
            + (cursor_index === index && window_has_focus ? " bg-accent/10 hover:bg-accent/10" : "")
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
                <Icon icon="mdi:content-cut" class="w-4 h-4" />
              {:else}
                <Icon icon={dir_icon()} class="w-4 h-4" />
              {/if}
            </span>
            <span class="text-print font-medium">{entry.name}</span>
          {:else}
            <span class="text-accent-detail">
              {#if clipboard?.entry.path === entry.path}
                <Icon icon="mdi:content-cut" class="w-4 h-4" />
              {:else}
                <Icon icon={file_icon(entry.name)} class="w-4 h-4" />
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
