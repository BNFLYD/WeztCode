<script>
  const changes = [
    { type: "M", color: "text-print-contrast", name: "App.svelte" },
    { type: "A", color: "text-accent-warn", name: "sidebar.svelte" },
  ];
</script>

<div class="space-y-4 py-4">
  <div class="bg-back rounded-xl p-4 flex flex-col gap-2">
    <p class="text-xs font-bold text-print uppercase tracking-wide">
      Cambios sin confirmar
    </p>
    <div class="space-y-1.5 text-xs">
      {#each changes as change}
        <div class="flex items-center gap-2">
          <span class="{change.color} font-bold">{change.type}</span>
          <span class="text-print">{change.name}</span>
        </div>
      {/each}
    </div>
  </div>
</div>
