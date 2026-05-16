<script>
  import Icon from "@iconify/svelte";

  let current_path = "/";
  let entries = [];
  let loading = true;
  let error = null;
  let expanded = new Map();

  async function load_dir(path) {
    loading = true;
    error = null;
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

  function toggle_dir(name) {
    const full_path = current_path === "/"
      ? name
      : `${current_path}/${name}`;
    if (expanded.has(full_path)) {
      expanded.delete(full_path);
      expanded = new Map(expanded);
    } else {
      fetch(`/api/fs/ls?path=${encodeURIComponent(full_path)}`)
        .then(r => r.json())
        .then(json => {
          if (json.ok) {
            expanded.set(full_path, json.data.files);
            expanded = new Map(expanded);
          }
        });
    }
  }

  function is_image(name) {
    return /\.(png|jpg|jpeg|gif|svg|webp)$/i.test(name);
  }

  function is_text(name) {
    return /\.(rs|toml|lua|js|ts|svelte|css|html|md|json|txt|yaml|yml|xml|sh|py|rb|go|c|h|cpp|hpp)$/i.test(name);
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
    return "lucide:file";
  }

  function dir_icon() {
    return "lucide:folder";
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

  load_dir("/");
</script>

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
    {#each entries as entry (entry.path)}
      <div class="flex items-center gap-2 px-3 py-1.5 rounded-lg hover:bg-accent/10 transition-colors text-lg group">
        {#if entry.entry_type === "dir"}
          <button
            class="flex items-center gap-2 flex-1 text-left"
            tabindex="0"
            on:click={() => open_dir(entry.name)}
            on:keydown={(e) => { if (e.key === 'Enter') open_dir(entry.name); }}
          >
            <span class="text-accent-detail">
              <Icon icon={dir_icon()} class="w-4 h-4" />
            </span>
            <span class="text-print font-medium">{entry.name}</span>
          </button>
        {:else}
          <button
            class="flex items-center gap-2 flex-1 text-left"
            tabindex="0"
            on:click={() => open_file(entry.path)}
            on:keydown={(e) => { if (e.key === 'Enter') open_file(entry.path); }}
          >
            <span class="text-accent-detail">
              <Icon icon={file_icon(entry.name)} class="w-4 h-4" />
            </span>
            <span class="text-print/70 flex-1 truncate">{entry.name}</span>
            {#if entry.size}
              <span class="text-print/30 text-xs">{Math.round(entry.size / 1024)}KB</span>
            {/if}
          </button>
        {/if}
      </div>
    {/each}
  {/if}
</div>
