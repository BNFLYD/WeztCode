<script>
  import Icon from "@iconify/svelte";
  import { onMount } from "svelte";

  // Secciones del sidebar
  let activeSection = "explorer";
  let expandedFolders = new Set();

  const sections = [
    { id: "explorer", icon: "solar:code-2-bold", label: "ファイル" },
    { id: "chat", icon: "mingcute:chat-4-line", label: "チャット" },
    { id: "git", icon: "mynaui:git-commit", label: "GIT" },
    { id: "view", icon: "streamline-plump:web", label: "ビュー" },
    { id: "term", icon: "devicon-plain:bash", label: "ターミナル" },
    {
      id: "settings",
      icon: "streamline-plump:compass-navigator-solid",
      label: "設定",
    },
  ];

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

  onMount(() => {
    console.log("[Svelte] onMount ejecutado");
  });
</script>

<!-- Sidebar -->
<div class="h-[100vh] flex flex-col bg-back-deep rounded-l-3xl overflow-hidden">
  <!-- Content Area -->
  <div class="flex-1 overflow-y-auto p-5">
    <!-- Explorer Section -->
    {#if activeSection === "explorer"}
      <div class="flex flex-col gap-1">
        {#each explorerItems as item (item.key)}
          <div style="padding-left: {item.level * 16}px">
            {#if item.folder}
              <button
                class="w-full flex items-center gap-2 px-3 py-2 rounded hover:bg-accent-detail/20 transition-colors group text-sm"
                on:click={() => toggleFolder(item.name)}
              >
                <span
                  class="text-accent-warn transition-transform"
                  class:rotate-[-90deg]={!item.isExpanded}
                >
                  <Icon icon="lucide:chevron-down" class="w-4 h-4" />
                </span>
                <span class="text-print/80 group-hover:text-print"
                  >{item.icon}</span
                >
                <span class="text-accent-warn/80 group-hover:text-accent-warn"
                  >{item.name}</span
                >
              </button>
            {:else}
              <div
                class="flex items-center gap-2 px-3 py-2 rounded hover:bg-accent-detail/20 transition-colors text-sm"
              >
                <span class="text-print">{item.icon}</span>
                <span class="text-accent-warn/70">{item.name}</span>
              </div>
            {/if}
          </div>
        {/each}
      </div>

      <!-- Chat Section -->
    {:else if activeSection === "chat"}
      <div class="space-y-4">
        <input
          type="text"
          placeholder="Buscar archivos..."
          class="w-full px-4 py-2.5 bg-back border border-accent-detail rounded text-sm text-accent-warn placeholder:text-accent-warn/50 outline-none focus:ring-2 focus:ring-[#2ca798]/20"
        />
        <div class="text-xs text-accent-warn/50 text-center py-8">
          Ingresa un término para buscar
        </div>
      </div>

      <!-- Git Section -->
    {:else if activeSection === "git"}
      <div class="space-y-4">
        <div
          class="bg-back border border-accent/30 rounded-lg p-4 flex flex-col gap-2"
        >
          <p class="text-xs font-bold text-print uppercase tracking-wide">
            Cambios sin confirmar
          </p>
          <div class="space-y-1.5 text-xs">
            <div class="flex items-center gap-2">
              <span class="text-accent-warn font-bold">M</span>
              <span class="text-print">App.svelte</span>
            </div>
            <div class="flex items-center gap-2">
              <span class="text-accent font-bold">A</span>
              <span class="text-print">sidebar.svelte</span>
            </div>
          </div>
        </div>
      </div>

      <!-- View Section -->
    {:else if activeSection === "view"}
      <div class="flex flex-col gap-2.5">
        {#each [1, 2, 3] as i}
          <div
            class="bg-back border border-accent/30 rounded-lg p-3.5 space-y-1 hover:bg-accent-detail/10 transition-colors cursor-pointer"
          >
            <p class="text-xs font-bold text-print">Notificación {i}</p>
            <p class="text-xs text-accent-warn/70">
              Mensaje de notificación importante
            </p>
          </div>
        {/each}
      </div>
    {:else if activeSection === "term"}
      <div class="flex flex-col gap-2.5">
        {#each [1, 2, 3] as i}
          <div
            class="bg-back border border-accent/30 rounded-lg p-3.5 space-y-1 hover:bg-accent-detail/10 transition-colors cursor-pointer"
          >
            <p class="text-xs font-bold text-print">Comando {i}</p>
            <p class="text-xs text-accent-warn/70">z /Projects/Rust/</p>
          </div>
        {/each}
      </div>

      <!-- Settings Section -->
    {:else if activeSection === "settings"}
      <div class="space-y-4">
        <div class="flex flex-col gap-2">
          <label class="text-xs font-bold text-print uppercase tracking-wide"
            >Tema</label
          >
          <select
            class="w-full px-3 py-2 bg-back border border-accent-detail rounded text-xs text-accent-warn outline-none"
          >
            <option>Oscuro</option>
            <option>Claro</option>
            <option>Automático</option>
          </select>
        </div>
        <div class="flex flex-col gap-2">
          <label class="text-xs font-bold text-print uppercase tracking-wide"
            >Tamaño de fuente</label
          >
          <input
            type="range"
            min="12"
            max="18"
            class="w-full h-1.5 bg-back rounded-lg appearance-none cursor-pointer accent-[#2ca798]"
          />
        </div>
      </div>
    {/if}
  </div>
  <!-- Footer Navigation -->
  <div
    class="border border-accent-detail rounded-2xl bg-back-deep flex flex-col p-2 mb-5 mx-1"
  >
    <div class="flex w-full justify-between px-4 py-2">
      {#each sections as section}
        <button
          class="isolate bg-accent-detail relative w-9 h-9 flex items-center justify-center rounded-md transition-all shadow-[inset_0_1px_0_rgba(255,255,255,0.10),0_2px_6px_rgba(0,0,0,0.25)] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.20),0_3px_8px_rgba(0,0,0,0.35)] active:shadow-[inset_0_4px_12px_rgba(0,0,0,0.55),inset_0_-1px_0_rgba(255,255,255,0.12)] hover:translate-y-[1px] active:translate-y-[3px] hover:bg-accent-detail/75 {activeSection ===
          section.id
            ? 'text-back shadow-[inset_0_4px_12px_rgba(0,255,255,0.7),inset_0_-1px_0_rgba(0,255,255,0.9)]'
            : 'text-back-deep'}"
          on:click={() => (activeSection = section.id)}
        >
          <Icon icon={section.icon} class="w-6 h-6" />
        </button>
      {/each}
    </div>
    <div class="flex justify-center -mb-5">
      <span
        class="bg-back-deep px-2 text-sm font-sans font-semibold tracking-[3px] text-print"
      >
        {sections.find((s) => s.id === activeSection)?.label}
      </span>
    </div>
  </div>
</div>
