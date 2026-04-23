<script>
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';

  // Secciones del sidebar
  let activeSection = 'explorer';
  let expandedFolders = new Set();

  const sections = [
    { id: 'explorer', icon: 'lucide:file-text', label: 'Explorer' },
    { id: 'search', icon: 'lucide:search', label: 'Search' },
    { id: 'git', icon: 'lucide:git-branch', label: 'Git' },
    { id: 'notifications', icon: 'lucide:bell', label: 'Notifications' },
    { id: 'settings', icon: 'lucide:settings', label: 'Settings' }
  ];

  const files = [
    { name: 'App.svelte', icon: '📄', level: 0 },
    { name: 'components', icon: '📁', level: 0, folder: true },
    { name: 'button.svelte', icon: '📄', level: 1 },
    { name: 'card.svelte', icon: '📄', level: 1 },
    { name: 'hooks', icon: '📁', level: 0, folder: true },
    { name: 'app.svelte.ts', icon: '📄', level: 1 }
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
      isExpanded: expandedFolders.has(file.name)
    }));
  }

  $: explorerItems = getExplorerItems();

  onMount(() => {
    console.log('[Svelte] onMount ejecutado');
  });
</script>

<div class="flex h-screen w-full bg-[#0d0d0d] text-[#2ca798]">
  <!-- Main Content Area -->
  <div class="flex-1 flex flex-col items-center justify-center border-r border-[#2ca798]/30 bg-gradient-to-br from-[#0d0d0d] via-[#1a1a2e] to-[#16213e]">
    <div class="text-center flex flex-col gap-4">
      <div class="text-6xl font-bold text-[#d0d0d0] uppercase tracking-[8px] drop-shadow-[0_0_20px_rgba(44,167,152,0.5)]">
        WeztCode
      </div>
      <p class="text-sm text-[#2ca798]/70 tracking-wider">Selecciona una sección en la sidebar</p>
    </div>
  </div>

  <!-- Sidebar Retrofuturista -->
  <div class="w-[30%] min-w-[300px] max-w-[400px] flex flex-col bg-[#1e1e1e] border-l-2 border-[#2ca798] rounded-l-3xl overflow-hidden">
    <!-- Section Header -->
    <div class="px-5 py-4 border-b border-[#2ca798]/30 bg-[#1e1e1e]/80">
      <h2 class="text-sm font-bold uppercase tracking-[3px] text-[#d0d0d0]">
        {sections.find(s => s.id === activeSection)?.label}
      </h2>
    </div>

    <!-- Content Area -->
    <div class="flex-1 overflow-y-auto p-5">
      <!-- Explorer Section -->
      {#if activeSection === 'explorer'}
        <div class="flex flex-col gap-1">
          {#each explorerItems as item (item.key)}
            <div style="padding-left: {item.level * 16}px">
              {#if item.folder}
                <button
                  class="w-full flex items-center gap-2 px-3 py-2 rounded hover:bg-[#2ca798]/20 transition-colors group text-sm"
                  on:click={() => toggleFolder(item.name)}
                >
                  <span class="text-[#2ca798] transition-transform" class:rotate-[-90deg]={!item.isExpanded}>
                    <Icon icon="lucide:chevron-down" class="w-4 h-4" />
                  </span>
                  <span class="text-[#d0d0d0]/80 group-hover:text-[#d0d0d0]">{item.icon}</span>
                  <span class="text-[#2ca798]/80 group-hover:text-[#2ca798]">{item.name}</span>
                </button>
              {:else}
                <div class="flex items-center gap-2 px-3 py-2 rounded hover:bg-[#2ca798]/20 transition-colors text-sm">
                  <span class="text-[#d0d0d0]">{item.icon}</span>
                  <span class="text-[#2ca798]/70">{item.name}</span>
                </div>
              {/if}
            </div>
          {/each}
        </div>

      <!-- Search Section -->
      {:else if activeSection === 'search'}
        <div class="space-y-4">
          <input
            type="text"
            placeholder="Buscar archivos..."
            class="w-full px-4 py-2.5 bg-[#0d0d0d] border border-[#2ca798] rounded text-sm text-[#2ca798] placeholder:text-[#2ca798]/50 outline-none focus:ring-2 focus:ring-[#2ca798]/20"
          />
          <div class="text-xs text-[#2ca798]/50 text-center py-8">Ingresa un término para buscar</div>
        </div>

      <!-- Git Section -->
      {:else if activeSection === 'git'}
        <div class="space-y-4">
          <div class="bg-[#0d0d0d] border border-[#2ca798]/30 rounded-lg p-4 flex flex-col gap-2">
            <p class="text-xs font-bold text-[#d0d0d0] uppercase tracking-wide">Cambios sin confirmar</p>
            <div class="space-y-1.5 text-xs">
              <div class="flex items-center gap-2">
                <span class="text-red-500 font-bold">M</span>
                <span class="text-[#d0d0d0]">App.svelte</span>
              </div>
              <div class="flex items-center gap-2">
                <span class="text-green-500 font-bold">A</span>
                <span class="text-[#d0d0d0]">sidebar.svelte</span>
              </div>
            </div>
          </div>
        </div>

      <!-- Notifications Section -->
      {:else if activeSection === 'notifications'}
        <div class="flex flex-col gap-2.5">
          {#each [1, 2, 3] as i}
            <div class="bg-[#0d0d0d] border border-[#2ca798]/30 rounded-lg p-3.5 space-y-1 hover:bg-[#2ca798]/10 transition-colors cursor-pointer">
              <p class="text-xs font-bold text-[#d0d0d0]">Notificación {i}</p>
              <p class="text-xs text-[#2ca798]/70">Mensaje de notificación importante</p>
            </div>
          {/each}
        </div>

      <!-- Settings Section -->
      {:else if activeSection === 'settings'}
        <div class="space-y-4">
          <div class="flex flex-col gap-2">
            <label class="text-xs font-bold text-[#d0d0d0] uppercase tracking-wide">Tema</label>
            <select class="w-full px-3 py-2 bg-[#0d0d0d] border border-[#2ca798] rounded text-xs text-[#2ca798] outline-none">
              <option>Oscuro</option>
              <option>Claro</option>
              <option>Automático</option>
            </select>
          </div>
          <div class="flex flex-col gap-2">
            <label class="text-xs font-bold text-[#d0d0d0] uppercase tracking-wide">Tamaño de fuente</label>
            <input type="range" min="12" max="18" class="w-full h-1.5 bg-[#0d0d0d] rounded-lg appearance-none cursor-pointer accent-[#2ca798]" />
          </div>
        </div>
      {/if}
    </div>

    <!-- Footer Navigation -->
    <div class="border-t border-[#2ca798]/30 bg-[#1e1e1e] p-4 flex items-center justify-center gap-2">
      {#each sections as section}
        <button
          class="w-9 h-9 flex items-center justify-center rounded-md transition-all {activeSection === section.id ? 'bg-[#2ca798] text-[#0d0d0d]' : 'text-[#2ca798]/70 hover:bg-[#2ca798]/15 hover:text-[#2ca798]'}"
          on:click={() => activeSection = section.id}
          title={section.label}
        >
          <Icon icon={section.icon} class="w-5 h-5" />
        </button>
      {/each}
    </div>
  </div>
</div>
