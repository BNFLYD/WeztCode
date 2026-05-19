<script>
  import Icon from "@iconify/svelte";

  export let icon = "";
  export let name = "";
  export let on_confirm = () => {};
  export let on_cancel = () => {};
  export let on_close = () => {};
</script>

<div class="absolute inset-0 flex items-center justify-center">
  <div class="bg-back-deep border border-accent-detail/30 rounded-xl p-4 shadow-lg mx-4 w-auto min-w-[200px]">
    <p class="text-sm text-print/70 mb-4">Queres borrar</p>
    <div class="flex items-center gap-2 mb-3 text-print">
      <Icon icon={icon} class="w-5 h-5 text-accent-detail" />
      <span class="font-mono text-sm truncate">{name}?</span>
    </div>
    <div class="flex gap-2 justify-end">
      <button
        on:click={() => { on_confirm(); on_close(); }}
        class="px-4 rounded-sm bg-back text-accent-detail hover:bg-accent-detail hover:text-back transition-colors border border-accent-detail text-sm"
      >
        Si
      </button>
      <button
        on:click={() => { on_cancel(); on_close(); }}
        class="px-4 rounded-sm bg-back text-accent-detail hover:bg-accent-detail hover:text-back transition-colors border border-accent-detail text-sm"
      >
        No
      </button>
    </div>
  </div>
</div>
