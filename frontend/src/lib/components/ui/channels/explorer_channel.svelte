<script>
  import Icon from "@iconify/svelte";

  export let icon = "";
  export let name = "";
  export let on_confirm = () => {};
  export let on_cancel = () => {};
  export let on_close = () => {};
</script>

<div class="absolute inset-0 flex items-center justify-center bg-black/60 z-50 rounded-2xl">
  <div class="bg-back-deep border border-accent-detail/30 rounded-xl p-4 shadow-lg mx-4 w-auto min-w-[200px]">
    <div class="flex items-center gap-2 mb-3 text-print">
      <Icon icon={icon} class="w-5 h-5 text-accent-detail" />
      <span class="font-mono text-sm truncate">{name}</span>
    </div>
    <p class="text-sm text-print/70 mb-4">Queres borrar?</p>
    <div class="flex gap-2 justify-end">
      <button
        on:click={() => { on_confirm(); on_close(); }}
        class="px-3 py-1 rounded-lg bg-accent/20 text-accent hover:bg-accent/30 transition-colors text-sm"
      >
        Si
      </button>
      <button
        on:click={() => { on_cancel(); on_close(); }}
        class="px-3 py-1 rounded-lg bg-accent-err/20 text-accent-err hover:bg-accent-err/30 transition-colors text-sm"
      >
        No
      </button>
    </div>
  </div>
</div>
