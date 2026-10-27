<script>
  import { onMount, onDestroy } from "svelte";
  import Icon from "@iconify/svelte";

  // Vista de diálogo Si/No compartida por los canales (explorer: borrar
  // archivo/carpeta/proyecto; chat: borrar conversación). Se renderiza en
  // el monitor. El teclado se escucha a nivel window: la vista solo
  // existe mientras está en pantalla, así que si el usuario clickea
  // afuera y pierde el foco, las teclas siguen vivas.
  let {
    icon = "",
    name = "",
    on_confirm = () => {},
    on_cancel = () => {},
    on_close = () => {},
  } = $props();

  let selected = $state("si");
  let dialog_ref = $state(null);

  function handle_keydown(e) {
    switch (e.key) {
      case "Enter":
      case " ":
        e.preventDefault();
        if (selected === "si") on_confirm();
        else on_cancel();
        on_close();
        break;
      case "Escape":
        e.preventDefault();
        on_cancel();
        on_close();
        break;
      case "ArrowLeft":
      case "h":
        e.preventDefault();
        selected = "si";
        break;
      case "ArrowRight":
      case "l":
        e.preventDefault();
        selected = "no";
        break;
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handle_keydown);
    dialog_ref?.focus();
  });

  onDestroy(() => {
    window.removeEventListener("keydown", handle_keydown);
  });
</script>

<div class="absolute inset-0 flex items-center justify-center">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={dialog_ref}
    tabindex="0"
    role="dialog"
    class="relative text-lg text-print font-sans bg-back-deep rounded-lg p-4 shadow-lg mx-7 w-auto min-w-[200px] outline-none"
  >
    <button
      onclick={() => on_close()}
      class="text-print/60 hover:text-print transition-colors shrink-0 absolute top-4 right-5"
    >
      <Icon icon="lucide:x" class="w-4 h-4" />
    </button>
    <p class="mb-1">Vas a borrar</p>
    <div class="flex items-center gap-2 text-print mb-4 ml-2">
      <Icon {icon} class="w-5 h-5 text-accent-detail" />
      <span class="font-bold truncate">{name}?</span>
    </div>
    <div class="flex px-2 gap-2 justify-between font-sans font-bold text-sm">
      <button
        onclick={() => {
          on_confirm();
          on_close();
        }}
        onmouseenter={() => (selected = "si")}
        class="px-5 rounded-sm transition-colors border border-accent-detail"
        class:bg-accent-detail={selected === "si"}
        class:text-back={selected === "si"}
        class:bg-transparent={selected !== "si"}
        class:text-print={selected !== "si"}
      >
        Si
      </button>
      <button
        onclick={() => {
          on_cancel();
          on_close();
        }}
        onmouseenter={() => (selected = "no")}
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
