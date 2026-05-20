<script>
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  export let mode = "confirm";
  export let icon = "";
  export let name = "";
  export let on_confirm = () => {};
  export let on_cancel = () => {};
  export let on_close = () => {};

  let selected = "si";
  let dialog_ref;

  function handle_keydown(e) {
    switch (e.key) {
      case "Enter":
      case " ":
        if (selected === "si") { on_confirm(); on_close(); }
        else { on_cancel(); on_close(); }
        break;
      case "Escape":
        on_cancel();
        on_close();
        break;
      case "ArrowLeft":
      case "h":
        selected = "si";
        break;
      case "ArrowRight":
      case "l":
        selected = "no";
        break;
    }
  }

  onMount(() => {
    dialog_ref?.focus();
  });
</script>

{#if mode === "confirm"}
  <div class="absolute inset-0 flex items-center justify-center">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      bind:this={dialog_ref}
      tabindex="0"
      role="dialog"
      on:keydown={handle_keydown}
      class="text-lg text-print font-sans bg-back-deep rounded-lg p-4 shadow-lg mx-7 w-auto min-w-[200px] outline-none"
    >
      <div class="flex items-center">
        <p class="mb-1">Queres borrar</p>
        <button
          on:click={() => { on_cancel(); on_close(); }}
          class="text-print/60 hover:text-print transition-colors shrink-0 absolute top-2 right-2"
        >
          <Icon icon="lucide:x" class="w-4 h-4" />
        </button>
      </div>
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
{:else if mode === "preview"}
  <div class="flex items-center gap-2 px-3 py-1.5 bg-back-deep/80 rounded-md text-sm">
    <Icon icon="lucide:image" class="w-4 h-4 text-accent-detail shrink-0" />
    <span class="text-print font-medium truncate flex-1">{name}</span>
  </div>
{/if}
