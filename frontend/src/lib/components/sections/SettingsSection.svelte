<script>
  let theme = "Dinamico";
  let fontSize = 14;
</script>

<div class="space-y-4 py-4">
  <div class="flex flex-col gap-2">
    <label class="text-xs font-bold text-print uppercase tracking-wide">
      Tema
      <select
        bind:value={theme}
        class="w-full px-3 py-2 bg-back border border-accent-detail rounded text-xs text-print-contrast outline-none mt-2"
      >
        <option>Oscuro</option>
        <option>Claro</option>
        <option>Dinamico</option>
      </select>
    </label>
  </div>
  <div class="flex flex-col gap-2">
    <label
      for="font-size"
      class="text-xs font-bold text-print uppercase tracking-wide">Tamano de fuente</label
    >
    <input
      id="font-size"
      type="range"
      min="12"
      max="18"
      bind:value={fontSize}
      class="w-full h-1.5 bg-back rounded-lg appearance-none cursor-pointer"
    />
  </div>
</div>
