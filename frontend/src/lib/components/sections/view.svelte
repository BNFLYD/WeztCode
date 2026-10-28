<script>
  import { onMount } from "svelte";
  import Icon from "@iconify/svelte";

  let { active_section = "view" } = $props();

  // Sección 表示: visualizador de URLs. Lista los servidores escuchando en
  // localhost (como la sección de terminales lista panes) y al hacer click
  // abre/reemplaza la ventana viewer (layer-shell al lado del menú).
  //
  // El usuario puede agregar sus propias URLs con la MISMA lógica que la
  // sección de terminales (input inline + cursor + teclas): en vez de
  // determinar una terminal, determina una URL. Las URLs del usuario viven
  // en localStorage porque la sección se desmonta al salir del menú.

  const STORE_KEY = "weztcode.viewer.targets";

  let detected = $state([]); // puertos escaneados: {url, label, port}
  let custom = $state([]); // URLs del usuario: {url}
  let loading = $state(true);
  let error = $state(null);
  let cursor_index = $state(0);
  let input_mode = $state(null); // null | "create" | "edit"
  let input_value = $state("");
  let edit_url = $state(null);
  let input_ref = $state(null);
  let list_ref = $state(null);
  let active_url = $state(null);

  // Lista unificada: primero lo escaneado, después lo del usuario.
  let targets = $derived([
    ...detected.map((d) => ({ ...d, custom: false })),
    ...custom.map((c) => ({ ...c, custom: true })),
  ]);

  // ---- URLs del usuario (persisten entre sesiones) ----
  function load_custom() {
    try {
      const raw = JSON.parse(localStorage.getItem(STORE_KEY) || "[]");
      return Array.isArray(raw) ? raw.filter((t) => t && t.url) : [];
    } catch {
      return [];
    }
  }

  function save_custom() {
    localStorage.setItem(STORE_KEY, JSON.stringify(custom));
  }

  // "5173" -> "http://localhost:5173/" | "localhost:5173/x" -> "http://localhost:5173/x"
  function normalize_url(raw) {
    let s = (raw || "").trim();
    if (!s) return null;
    if (/^\d+$/.test(s)) s = `localhost:${s}`;
    if (!/^https?:\/\//i.test(s)) s = `http://${s}`;
    if (!/^https?:\/\/[^/\s]+/i.test(s)) return null;
    if (!/^https?:\/\/[^/\s]+\//i.test(s)) s += "/";
    return s;
  }

  // ---- backend ----
  async function load_detected() {
    loading = true;
    try {
      const res = await fetch("/api/ports/list");
      const data = await res.json();
      detected = (data.data || []).map((p) => ({
        url: `http://localhost:${p.port}/`,
        label: p.process,
        port: p.port,
      }));
      error = null;
    } catch (e) {
      error = e?.message ?? "Error al escanear puertos";
      detected = [];
    } finally {
      loading = false;
    }
  }

  async function load_status() {
    try {
      const res = await fetch("/api/ports/status");
      const data = await res.json();
      active_url = data.data?.url ?? null;
    } catch {}
  }

  async function open_url(url) {
    error = null;
    try {
      const res = await fetch("/api/ports/open", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ url }),
      });
      const data = await res.json();
      if (data.ok) {
        active_url = url;
      } else {
        error = data.error ?? "No se pudo abrir la URL";
      }
    } catch (e) {
      error = e?.message ?? "Error de conexión";
    }
  }

  async function close_viewer() {
    try {
      await fetch("/api/ports/close", { method: "POST" });
    } catch {}
    active_url = null;
  }

  // ---- input de URL (create / edit) ----
  function start_create() {
    input_mode = "create";
    input_value = "";
    edit_url = null;
  }

  function start_edit() {
    const target = targets[cursor_index];
    if (!target || !target.custom) {
      error = "Solo las URLs agregadas se pueden editar";
      return;
    }
    input_mode = "edit";
    input_value = target.url;
    edit_url = target.url;
  }

  function cancel_input() {
    input_mode = null;
    input_value = "";
    edit_url = null;
  }

  async function submit_input() {
    const url = normalize_url(input_value);
    if (!url) {
      error = "URL inválida (ej: localhost:5173)";
      return;
    }
    if (custom.some((t) => t.url === url && t.url !== edit_url)) {
      error = "Esa URL ya está en la lista";
      return;
    }
    if (input_mode === "edit") {
      custom = custom.map((t) => (t.url === edit_url ? { ...t, url } : t));
    } else {
      custom = [...custom, { url }];
    }
    save_custom();
    cancel_input();
    cursor_index = targets.findIndex((t) => t.url === url);
    await open_url(url);
  }

  function delete_current() {
    const target = targets[cursor_index];
    if (!target || !target.custom) return;
    custom = custom.filter((t) => t.url !== target.url);
    save_custom();
    cursor_index = Math.min(cursor_index, Math.max(targets.length - 1, 0));
  }

  // ---- cursor + teclado (misma lógica que la sección de terminales) ----
  function move_cursor(delta) {
    const next = cursor_index + delta;
    if (next < 0 || next >= targets.length) return;
    cursor_index = next;
    scroll_to_cursor();
  }

  function scroll_to_cursor() {
    if (!list_ref) return;
    const child = list_ref.querySelector(`[data-index="${cursor_index}"]`);
    if (child) child.scrollIntoView({ block: "nearest" });
  }

  function activate_current() {
    const target = targets[cursor_index];
    if (target) open_url(target.url);
  }

  function handle_keydown(e) {
    if (!document.hasFocus()) return;
    if (active_section !== "view") return;

    if (input_mode) {
      if (e.key === "Enter") {
        e.preventDefault();
        submit_input();
      } else if (e.key === "Escape") {
        e.preventDefault();
        cancel_input();
      }
      return;
    }

    switch (e.key) {
      case "j":
      case "ArrowDown":
        e.preventDefault();
        move_cursor(1);
        break;
      case "k":
      case "ArrowUp":
        e.preventDefault();
        move_cursor(-1);
        break;
      case "l":
      case "Enter":
        e.preventDefault();
        activate_current();
        break;
      case "a":
      case "A":
        e.preventDefault();
        start_create();
        break;
      case "r":
      case "R":
        e.preventDefault();
        start_edit();
        break;
      case "d":
      case "D":
        e.preventDefault();
        delete_current();
        break;
    }
  }

  $effect(() => {
    if (input_mode && input_ref) input_ref.focus();
  });

  // La sección se remonta al entrar: scan fresco + estado de la ventana.
  onMount(() => {
    custom = load_custom();
    load_detected();
    load_status();
  });
</script>

<svelte:window onkeydown={handle_keydown} />

<div class="flex flex-col gap-2 py-3">
  <!-- header: + (agregar URL) / input inline / estado del viewer / refresh -->
  <div class="flex items-center gap-2 px-1">
    <button
      onclick={start_create}
      class="flex items-center gap-1 text-xs text-print/50 hover:text-print transition-colors"
      title="Agregar URL (a)"
    >
      <Icon icon="tabler:plus-filled" class="w-4 h-4" />
    </button>

    {#if input_mode}
      <span class="font-mono truncate flex items-center gap-1 text-print flex-1 min-w-0">
        <span class="text-print-contrast text-sm font-semibold">{input_mode === "create" ? "$_" : ">_"}</span>
        <input
          bind:value={input_value}
          bind:this={input_ref}
          placeholder="localhost:5173"
          class="bg-transparent outline-none text-print font-mono text-sm flex-1 min-w-0"
          onblur={() => {
            if (input_mode === "create" && !input_value.trim()) cancel_input();
          }}
        />
      </span>
    {:else}
      <span class="text-xs font-bold text-print uppercase tracking-wide flex-1">URLs</span>
    {/if}

    {#if active_url}
      <button
        class="flex items-center gap-1 text-xs px-2 py-1 rounded-lg bg-accent-detail/20 text-print hover:bg-accent-detail/40 transition-colors max-w-[45%]"
        onclick={close_viewer}
        title="Cerrar ventana viewer"
      >
        <Icon icon="mdi:window-close" class="w-3.5 h-3.5 shrink-0" />
        <span class="font-mono truncate">{active_url}</span>
      </button>
    {/if}

    <button
      class="text-print/50 hover:text-print transition-colors"
      onclick={load_detected}
      title="Reescanear puertos"
    >
      <Icon icon="mdi:refresh" class="w-4 h-4 {loading ? 'animate-spin' : ''}" />
    </button>
  </div>

  {#if error}
    <p class="text-xs text-accent-err px-1">{error}</p>
  {/if}

  {#if loading && targets.length === 0}
    <div class="flex items-center justify-center py-10">
      <span class="text-print/50 text-lg">Loading...</span>
    </div>
  {:else if targets.length === 0}
    <div class="flex items-center justify-center py-10">
      <div class="text-center text-print/50">
        <Icon icon="mdi:earth-off" class="w-14 h-14 mx-auto mb-2 opacity-50" />
        <p class="text-md">No hay URLs</p>
        <p class="text-xs mt-1">Levantá un server (vite, node, flask...) o agregá una URL con +</p>
      </div>
    </div>
  {:else}
    <div class="flex-1 overflow-y-auto min-h-0" bind:this={list_ref}>
      {#each targets as t, index (t.custom ? `c:${t.url}` : `d:${t.url}`)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="bg-back rounded-xl p-3.5 space-y-1 transition-colors w-full text-left cursor-pointer
                 {index === cursor_index ? 'ring-1 ring-accent-detail bg-accent-detail/10' : 'hover:bg-accent-detail/10'}"
          data-index={index}
          onclick={() => {
            cursor_index = index;
            activate_current();
          }}
        >
          <div class="flex items-center gap-2">
            <Icon
              icon={t.custom ? "mdi:bookmark-outline" : "mdi:web"}
              class="w-4 h-4 text-accent-detail shrink-0"
            />
            <p class="text-xs font-bold text-print truncate flex-1">
              {t.custom ? t.url : t.label}
            </p>
            {#if t.url === active_url}
              <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-accent-detail text-back font-bold">ACTIVO</span>
            {/if}
          </div>
          <p class="text-xs text-print-contrast/70 font-mono ml-6 truncate">
            {t.custom ? "agregada" : `:${t.port}`}
          </p>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  button:focus-visible {
    outline: none;
  }
</style>
