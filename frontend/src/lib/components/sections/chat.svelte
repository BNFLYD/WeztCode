<script>
  import Icon from "@iconify/svelte";

  let input_value = "";
</script>

<div class="flex flex-col h-full">
  <div class="flex-1 overflow-y-auto">
    <!-- Chat messages would go here -->
  </div>
  <div class="flex flex-col">
    <div class="flex mt-auto pt-4 pb-2 w-full items-stretch">
      <!-- Input -->
      <div
        class="-mr-2 flex-1 relative px-5 py-6 bg-transparent rounded-l-lg flex items-center justify-center overflow-hidden group"
      >
        <!-- Background diagonal input -->
        <div
          class="rounded-lg absolute inset-0 bg-back -translate-x-[4%] skew-x-[-20deg] origin-left"
        >
          <input
            type="text"
            placeholder="Decime lo que pensás..."
            bind:value={input_value}
            class="w-full skew-x-[20deg] pl-4 pr-2 bg-transparent rounded-lg text-lg text-print-contrast placeholder:text-print-contrast/50 outline-none"
          />
        </div>
      </div>

      <!-- Button -->
      <div
        class="relative pl-5 pr-3 -ml-2 bg-transparent rounded-r-lg flex items-center justify-center overflow-hidden group"
      >
        <!-- Background diagonal button -->
        <button
          class="rounded-lg absolute inset-0 bg-accent-detail translate-x-[14%] skew-x-[-20deg] origin-right"
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
