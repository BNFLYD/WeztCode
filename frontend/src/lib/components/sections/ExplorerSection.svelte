<script>
  import Icon from "@iconify/svelte";

  let expandedFolders = new Set();

  const files = [
    { name: "App.svelte", icon: "📄", level: 0 },
    { name: "components", icon: "📁", level: 0, folder: true },
    { name: "button.svelte", icon: "📄", level: 1 },
    { name: "card.svelte", icon: "📄", level: 1 },
    { name: "hooks", icon: "📁", level: 0, folder: true },
    { name: "app.svelte.ts", icon: "📄", level: 1 },
  ];

  function toggleFolder(name) {
    if (expandedFolders.has(name)) {
      expandedFolders.delete(name);
    } else {
      expandedFolders.add(name);
    }
    expandedFolders = expandedFolders;
  }

  function getExplorerItems() {
    return files.map((file, idx) => ({
      ...file,
      key: `${file.name}-${idx}`,
      isExpanded: expandedFolders.has(file.name),
    }));
  }

  $: explorerItems = getExplorerItems();
</script>

<div class="flex flex-col gap-1 py-4">
  {#each explorerItems as item (item.key)}
    <div style="padding-left: {item.level * 16}px">
      {#if item.folder}
        <button
          class="w-full flex items-center gap-2 px-3 py-2 rounded hover:bg-accent/20 transition-colors group text-md text-print font-semibold"
          on:click={() => toggleFolder(item.name)}
        >
          <span
            class="text-print transition-transform"
            class:rotate-[-90deg]={!item.isExpanded}
          >
            <Icon icon="lucide:chevron-down" class="w-4 h-4" />
          </span>
          <span class="text-print/80 group-hover:text-print">{item.icon}</span>
          <span class="text-print/80 group-hover:text-print">{item.name}</span>
        </button>
      {:else}
        <div
          class="flex items-center gap-2 px-3 py-2 rounded hover:bg-accent-detail/20 transition-colors text-sm"
        >
          <span class="text-print">{item.icon}</span>
          <span class="text-print/70">{item.name}</span>
        </div>
      {/if}
    </div>
  {/each}
</div>
