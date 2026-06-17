<script>
  export let on_navigate_to_explorer;

  let theme = "Dinamico";
  let font_size = 14;

  let keys = [];
  let show_add_form = false;
  let new_name = "";
  let new_value = "";
  let edit_key = null;
  let edit_value = "";
  let error_msg = "";

  async function load_keys() {
    try {
      const res = await fetch("/api/keys/list");
      const data = await res.json();
      keys = data.keys || [];
    } catch {
      keys = [];
    }
  }

  async function set_key(name, value) {
    error_msg = "";
    try {
      const res = await fetch("/api/keys/set", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name, value }),
      });
      const data = await res.json();
      if (!data.ok) {
        error_msg = data.error || "Error al guardar la key";
        return;
      }
      await load_keys();
    } catch {
      error_msg = "Error de conexion";
    }
  }

  async function delete_key(name) {
    if (!confirm(`Eliminar ${name}?`)) return;
    error_msg = "";
    try {
      const res = await fetch(`/api/keys/delete?name=${encodeURIComponent(name)}`);
      const data = await res.json();
      if (!data.ok) {
        error_msg = data.error || "Error al eliminar";
        return;
      }
      await load_keys();
    } catch {
      error_msg = "Error de conexion";
    }
  }

  function start_edit(name) {
    edit_key = name;
    edit_value = "";
  }

  async function save_edit() {
    if (!edit_key || !edit_value) return;
    await set_key(edit_key, edit_value);
    edit_key = null;
    edit_value = "";
  }

  async function add_key() {
    if (!new_name || !new_value) return;
    await set_key(new_name, new_value);
    new_name = "";
    new_value = "";
    show_add_form = false;
  }

  async function open_default_terms() {
    const res = await fetch('/api/terminal/active-pane');
    const json = await res.json();
    if (json.ok && json.data.pane_id !== 0) {
      await fetch('/api/terminal/ensure-main', { method: 'POST' });
    }
    await fetch("/api/terminal/edit-defaults");
  }

  async function open_models_editor() {
    const res = await fetch('/api/terminal/active-pane');
    const json = await res.json();
    if (json.ok && json.data.pane_id !== 0) {
      await fetch('/api/terminal/ensure-main', { method: 'POST' });
    }
    await fetch("/api/models/edit-defaults");
  }

  async function edit_agents() {
    const res = await fetch('/api/sub-agents/dirs');
    const json = await res.json();
    if (json.ok) {
      const primary = json.data.primary;
      await fetch('/api/projects/add', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: primary })
      });
      await fetch(`/api/projects/switch?path=${encodeURIComponent(primary)}`);
      on_navigate_to_explorer("/");
    }
  }

  load_keys();
</script>

<div class="space-y-4 py-4">
  <div class="flex flex-col gap-2">
    <label class="text-md font-bold text-print uppercase tracking-wide">
      Tema
      <select
        bind:value={theme}
        class="w-full px-3 py-2 bg-back border border-accent-detail rounded text-xs text-print-contrast outline-none mt-2"
      >
        <option>Oscuro</option>
        <option>Claro</option>
        <option>Dinamico</option>
      </select>
    </label>
  </div>

  <hr class="border-accent/50 my-2" />

  <div class="flex flex-col gap-3">
    <h3 class="text-md font-bold text-print uppercase tracking-wide">API Keys</h3>
    <p class="text-md text-print">
      Las keys se almacenan en ~/.config/weztcode/preferences/models/KEYS.env fuera del proyecto.
      Los valores no se muestran por seguridad.
    </p>

    {#if error_msg}
      <p class="text-[10px] text-red-500">{error_msg}</p>
    {/if}

    {#if keys.length === 0 && !show_add_form}
      <p class="text-md text-print italic">No hay keys configuradas</p>
    {/if}

    <div class="flex flex-col gap-1">
      {#each keys as name}
        <div class="flex items-center justify-between px-3 py-2 bg-back rounded">
          <span class="text-xs text-print-contrast font-mono">{name}</span>
          <div class="flex gap-2">
            <button
              on:click={() => start_edit(name)}
              class="text-sm text-print hover:text-accent/50"
              title="Reemplazar key"
            >reemplazar</button>
            <button
              on:click={() => delete_key(name)}
              class="text-sm text-print hover:text-accent-dng/50"
              title="Eliminar key"
            >eliminar</button>
          </div>
        </div>
        {#if edit_key === name}
          <div class="flex gap-2 px-3 pb-2">
            <input
              type="password"
              bind:value={edit_value}
              placeholder="Nuevo valor"
              class="flex-1 px-2 py-1 bg-back border border-accent-detail rounded text-xs text-print-contrast outline-none"
            />
            <button
              on:click={save_edit}
              class="font-semibold text-sm px-3 py-1.5 bg-back rounded text-print self-start hover:bg-accent/50 hover:text-back"
            >guardar</button>
          </div>
        {/if}
      {/each}
    </div>

    {#if show_add_form}
      <div class="flex flex-col gap-2 px-3 py-2 bg-back rounded">
        <input
          bind:value={new_name}
          placeholder="Nombre (ej: OPENROUTER)"
          class="w-full px-2 py-1 bg-back-deep rounded text-sm text-print-contrast outline-none"
        />
        <input
          type="password"
          bind:value={new_value}
          placeholder="API key"
          class="w-full px-2 py-1 bg-back-deep rounded text-sm text-print-contrast outline-none"
        />
        <div class="flex gap-2 justify-end">
          <button
            on:click={() => { show_add_form = false; error_msg = ""; }}
            class="text-md text-print"
          >Cancelar</button>
          <button
            on:click={add_key}
            class="font-semibold text-sm px-3 py-1.5 bg-back rounded text-print self-start hover:bg-accent/50 hover:text-back"
          >Guardar</button>
        </div>
      </div>
    {:else}
      <button
        on:click={() => { show_add_form = true; error_msg = ""; }}
        class="font-semibold text-sm px-3 py-1.5 bg-back rounded text-print self-start hover:bg-accent/50 hover:text-back"
      >+ agregar key</button>
    {/if}
  </div>

  <hr class="border-accent/50 my-2" />

  <div class="flex flex-col gap-3">
    <h3 class="text-md font-bold text-print uppercase tracking-wide">
      Terminales por defecto
    </h3>
    <p class="text-md text-print">
      Editá el archivo JSON para definir terminales que se abren al inicio.
      Se spawnearán automáticamente al reiniciar WeztCode.
    </p>
    <button
      on:click={open_default_terms}
      class="font-semibold text-sm px-3 py-1.5 bg-back rounded text-print self-start hover:bg-accent/50 hover:text-back"
    >Editar terminales</button>
  </div>

  <hr class="border-accent/50 my-2" />

  <div class="flex flex-col gap-3">
    <h3 class="text-md font-bold text-print uppercase tracking-wide">
      Modelos de IA
    </h3>
    <p class="text-md text-print">
      Editá el archivo JSON para definir los modelos disponibles en el chat.
      El modelo con "default": true se usa al iniciar la app.
    </p>
    <button
      on:click={open_models_editor}
      class="font-semibold text-sm px-3 py-1.5 bg-back rounded text-print self-start hover:bg-accent/50 hover:text-back"
    >Editar modelos</button>
  </div>

  <hr class="border-accent/50 my-2" />

  <div class="flex flex-col gap-3">
    <h3 class="text-md font-bold text-print uppercase tracking-wide">
      Sub-Agentes
    </h3>
    <p class="text-md text-print">
      Los sub-agentes son roles de IA (coder, writer, reviewer) definidos en archivos .md
      con su propio system prompt y modelo. Editá los agentes en <code>~/.pi/agent/agents/</code>.
    </p>
    <div class="flex gap-2">
      <button
        on:click={edit_agents}
        class="font-semibold text-sm px-3 py-1.5 bg-back rounded text-print self-start hover:bg-accent/50 hover:text-back"
      >Editar agentes</button>
    </div>
  </div>
</div>
