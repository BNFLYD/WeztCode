<script>
  const terminals = [
    { id: 1, name: "Bash Script 1", path: "z /Projects/Rust/" },
    { id: 2, name: "Bash Script 2", path: "z /Projects/Rust/" },
    { id: 3, name: "Bash Script 3", path: "z /Projects/Rust/" },
  ];
</script>

<div class="flex flex-col gap-2.5 py-4">
  {#each terminals as terminal}
    <div
      class="bg-back rounded-xl p-3.5 space-y-1 hover:bg-accent-detail/10 transition-colors cursor-pointer"
    >
      <p class="text-xs font-bold text-print">{terminal.name}</p>
      <p class="text-xs text-print-contrast/70">{terminal.path}</p>
    </div>
  {/each}
</div>
