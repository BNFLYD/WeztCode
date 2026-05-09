<script>
  import Icon from "@iconify/svelte";

  let inputValue = "";
</script>

<div class="flex flex-col h-full">
  <div class="flex-1 overflow-y-auto">
    <!-- Chat messages would go here -->
  </div>
  <div class="flex flex-col">
    <div class="flex mt-auto pt-4 pb-2 w-full items-stretch">
      <!-- Input -->
      <div
        class="-mr-2 flex-1 relative px-5 py-6 bg-transparent rounded-l-xl flex items-center justify-center overflow-hidden group"
      >
        <!-- Diagonal de fondo -->
        <div
          class="rounded-lg absolute inset-0 bg-back -translate-x-[4%] skew-x-[-20deg] origin-left"
        >
          <input
            type="text"
            placeholder="Decime lo que pensás..."
            bind:value={inputValue}
            class="w-full skew-x-[20deg] pl-4 pr-2 bg-transparent rounded-xl text-sm text-print-contrast placeholder:text-print-contrast/50 outline-none"
          />
        </div>
      </div>

      <!-- Boton con corte diagonal -->
      <div
        class="relative px-4 -ml-2 bg-transparent rounded-r-xl flex items-center justify-center overflow-hidden group"
      >
        <!-- Diagonal de fondo -->
        <button
          class="rounded-xl absolute inset-0 bg-accent-detail translate-x-[14%] skew-x-[-20deg] origin-right"
          aria-label="Enviar"
        ></button>

        <!-- Icono -->
        <Icon
          icon="mingcute:navigation-fill"
          class="text-back-deep w-6 h-6 relative z-10 transition-transform group-active:scale-75"
        />
      </div>
    </div>
  </div>
</div>
