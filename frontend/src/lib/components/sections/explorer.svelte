<script>
  import Icon from "@iconify/svelte";

  let current_path = "/";
  let entries = [];
  let loading = true;
  let error = null;
  let cursor_index = 0;
  let list_ref;

  async function load_dir(path) {
    loading = true;
    error = null;
    cursor_index = 0;
    try {
      const res = await fetch(`/api/fs/ls?path=${encodeURIComponent(path)}`);
      const json = await res.json();
      if (json.ok) {
        entries = json.data.files;
        current_path = json.data.path;
      } else {
        error = json.error;
      }
    } catch (e) {
      error = e.message;
    }
    loading = false;
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
    return "tabler:file-filled";
  }

  function dir_icon() {
    return "mdi:folder";
  }

  function open_file(path) {
    fetch(`/api/editor/open?path=${encodeURIComponent(path)}`);
  }

  function open_dir(name) {
    load_dir(current_path === "/" ? name : `${current_path}/${name}`);
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
    if (list_ref) {
      const child = list_ref.children[cursor_index];
      if (child) child.scrollIntoView({ block: "nearest" });
    }
  }

  function activate_current() {
    const entry = entries[cursor_index];
    if (!entry) return;
    if (entry.entry_type === "dir") open_dir(entry.name);
    else open_file(entry.path);
  }

  function handle_keydown(e) {
    if (!list_ref?.contains(e.target)) return;
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
      case "Enter":
      case " ":
        e.preventDefault();
        activate_current();
        break;
    }
  }

  load_dir("/");
</script>

<svelte:window on:keydown={handle_keydown} />

<div class="flex flex-col gap-1 py-4">
  <div class="flex items-center gap-2 px-3 py-2 text-xs text-print/50 border-b border-accent-detail/20 mb-2">
    <button on:click={go_up} class="hover:text-print transition-colors" disabled={current_path === "/"}>
      <Icon icon="lucide:arrow-up" class="w-4 h-4" />
    </button>
    <span class="font-mono truncate">{current_path}</span>
  </div>

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
    <!-- svelte-ignore a11y-no-static-element-interactions -->
    <div class="flex flex-col gap-1" tabindex="-1" bind:this={list_ref}>
      {#each entries as entry, index (entry.path)}
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <div
          class="flex items-center gap-2 px-3 py-1.5 rounded-lg hover:bg-accent/10 transition-colors text-lg group cursor-pointer"
          class:bg-accent/20={cursor_index === index}
          on:click={() => {
            if (entry.entry_type === "dir") open_dir(entry.name);
            else open_file(entry.path);
            list_ref?.focus();
          }}
        >
          {#if entry.entry_type === "dir"}
            <span class="text-accent-detail">
              <Icon icon={dir_icon()} class="w-4 h-4" />
            </span>
            <span class="text-print font-medium">{entry.name}</span>
          {:else}
            <span class="text-accent-detail">
              <Icon icon={file_icon(entry.name)} class="w-4 h-4" />
            </span>
            <span class="text-print/70 flex-1 truncate">{entry.name}</span>
            {#if entry.size}
              <span class="text-print/30 text-xs">{Math.round(entry.size / 1024)}KB</span>
            {/if}
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
