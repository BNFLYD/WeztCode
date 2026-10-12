<script>
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  // Sección 表示: visualizador de puertos. Lista los servidores escuchando
  // en localhost (como la sección de terminales lista panes) y al hacer
  // click abre/reemplaza la ventana viewer (layer-shell al lado del menú).

  let ports = $state([]);
  let active_port = $state(null); // puerto que la ventana está mostrando
  let loading = $state(true);
  let error = $state(null);

  async function load_ports() {
    loading = true;
    try {
      const res = await fetch("/api/ports/list");
      const data = await res.json();
      ports = data.data || [];
      error = null;
    } catch (e) {
      error = e?.message ?? "Error al escanear puertos";
      ports = [];
    } finally {
      loading = false;
    }
  }

  async function load_status() {
    try {
      const res = await fetch("/api/ports/status");
      const data = await res.json();
      active_port = data.data?.port ?? null;
    } catch {}
  }

  async function open_port(port) {
    try {
      const res = await fetch("/api/ports/open", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ port }),
      });
      const data = await res.json();
      if (data.ok) {
        active_port = port;
      } else {
        error = data.error ?? "No se pudo abrir el puerto";
      }
    } catch (e) {
      error = e?.message ?? "Error de conexión";
    }
  }

  async function close_viewer() {
    try {
      await fetch("/api/ports/close", { method: "POST" });
    } catch {}
    active_port = null;
  }

  // La sección se remonta al entrar: scan fresco + estado de la ventana.
  onMount(() => {
    load_ports();
    load_status();
  });
</script>

<div class="flex flex-col gap-2.5 py-4">
  <!-- header: título + estado del viewer + refresh manual -->
  <div class="flex items-center gap-2 px-1">
    <span class="text-xs font-bold text-print uppercase tracking-wide flex-1">Puertos</span>
    {#if active_port}
      <button
        class="flex items-center gap-1 text-xs px-2 py-1 rounded-lg bg-accent-detail/20 text-print hover:bg-accent-detail/40 transition-colors"
        onclick={close_viewer}
        title="Cerrar ventana viewer"
      >
        <Icon icon="mdi:window-close" class="w-3.5 h-3.5" />
        :{active_port}
      </button>
    {/if}
    <button
      class="text-print/50 hover:text-print transition-colors"
      onclick={load_ports}
      title="Reescanear puertos"
    >
      <Icon icon="mdi:refresh" class="w-4 h-4 {loading ? 'animate-spin' : ''}" />
    </button>
  </div>

  {#if error}
    <p class="text-xs text-red-400 px-1">{error}</p>
  {/if}

  {#if !loading && ports.length === 0}
    <div class="flex items-center justify-center py-10">
      <div class="text-center text-print/50">
        <Icon icon="mdi:earth-off" class="w-14 h-14 mx-auto mb-2 opacity-50" />
        <p class="text-md">No hay puertos escuchando</p>
        <p class="text-xs mt-1">Levantá un server (vite, node, flask...) y refrescá</p>
      </div>
    </div>
  {:else}
    {#each ports as p (p.port)}
      <button
        class="bg-back rounded-xl p-3.5 space-y-1 transition-colors w-full text-left
               {p.port === active_port ? 'ring-1 ring-accent-detail bg-accent-detail/10' : 'hover:bg-accent-detail/10'}"
        onclick={() => open_port(p.port)}
      >
        <div class="flex items-center gap-2">
          <Icon icon="mdi:web" class="w-4 h-4 text-accent-detail shrink-0" />
          <p class="text-xs font-bold text-print truncate flex-1">{p.process}</p>
          {#if p.port === active_port}
            <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-accent-detail text-back font-bold">ACTIVO</span>
          {/if}
        </div>
        <p class="text-xs text-print-contrast/70 font-mono ml-6">:{p.port}</p>
      </button>
    {/each}
  {/if}
</div>

<style>
  button:focus-visible {
    outline: none;
  }
</style>
