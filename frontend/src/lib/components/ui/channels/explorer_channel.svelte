<script>
  import Icon from "@iconify/svelte";

  export let icon = "";
  export let name = "";
  export let on_confirm = () => {};
  export let on_cancel = () => {};
  export let on_close = () => {};

  let selected = "si";
</script>

<div class="absolute inset-0 flex items-center justify-center">
  <div class="text-lg text-print font-sans bg-back-deep rounded-lg p-4 shadow-lg mx-7 w-auto min-w-[200px]">
    <p class="mb-1">Queres borrar</p>
    <div class="flex items-center gap-2 text-print mb-4 ml-2">
      <Icon icon={icon} class="w-5 h-5 text-accent-detail"/>
      <span class="font-bold truncate">{name}?</span>
    </div>
    <div class="flex px-2 gap-2 justify-between font-sans font-bold text-sm">
      <button
        on:click={() => { on_confirm(); on_close(); }}
        on:mouseenter={() => selected = "si"}
        class="px-5 rounded-sm transition-colors border border-accent-detail"
        class:bg-accent-detail={selected === "si"}
        class:text-back={selected === "si"}
        class:bg-transparent={selected !== "si"}
        class:text-print={selected !== "si"}
      >
        Si
      </button>
      <button
        on:click={() => { on_cancel(); on_close(); }}
        on:mouseenter={() => selected = "no"}
        class="px-5 rounded-sm transition-colors border border-accent-detail"
        class:bg-accent-detail={selected === "no"}
        class:text-back={selected === "no"}
        class:bg-transparent={selected !== "no"}
        class:text-print={selected !== "no"}
      >
        No
      </button>
    </div>
  </div>
</div>
