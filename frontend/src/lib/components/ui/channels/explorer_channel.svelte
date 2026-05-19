<script>
  import Icon from "@iconify/svelte";

  export let icon = "";
  export let name = "";
  export let on_confirm = () => {};
  export let on_cancel = () => {};
  export let on_close = () => {};
</script>

<div class="absolute inset-0 flex items-center justify-center">
  <div class="text-lg text-print font-sans bg-back-deep rounded-xl p-4 shadow-lg mx-7 w-auto min-w-[200px]">
    <p class="mb-1">Queres borrar</p>
    <div class="flex items-center gap-2 text-print mb-4 ml-2">
      <Icon icon={icon} class="w-5 h-5 text-accent-detail"/>
      <span class="font-bold truncate">{name}?</span>
    </div>
    <div class="flex px-4 gap-2 justify-between font-sans text-md">
      <button
        on:click={() => { on_confirm(); on_close(); }}
        class="px-5 rounded-md bg-back text-print hover:bg-accent-detail hover:text-back transition-colors border border-accent-detail"
      >
        Si
      </button>
      <button
        on:click={() => { on_cancel(); on_close(); }}
        class="px-5 rounded-md bg-back text-print hover:bg-accent-detail hover:text-back transition-colors border border-accent-detail"
      >
        No
      </button>
    </div>
  </div>
</div>
