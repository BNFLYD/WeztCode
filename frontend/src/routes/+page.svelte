<script>
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let termMsg = $state("WezTerm iniciando...");
  let isExpanded = $state(true);
  let paneList = $state("");
  let commandText = $state("ls -la");
  const EXPANDED_WIDTH = 350;
  const COLLAPSED_WIDTH = 50;

  async function toggleSidebar() {
    const window = getCurrentWindow();
    isExpanded = !isExpanded;
    const newWidth = isExpanded ? EXPANDED_WIDTH : COLLAPSED_WIDTH;
    await window.setSize({ width: newWidth, height: 800 });
  }

  async function spawnTerm() {
    termMsg = await invoke("spawn_term");
  }

  async function listPanes() {
    try {
      paneList = await invoke("wezterm_list");
    } catch (e) {
      paneList = "Error: " + e;
    }
  }

  async function sendCommand() {
    try {
      termMsg = await invoke("wezterm_send_text", { text: commandText + "\n" });
    } catch (e) {
      termMsg = "Error: " + e;
    }
  }
</script>

<main class="sidebar" class:collapsed={!isExpanded}>
  <header>
    {#if isExpanded}
      <h2>WeztCode</h2>
    {/if}
    <button class="toggle-btn" on:click={toggleSidebar} title={isExpanded ? "Colapsar" : "Expandir"}>
      {isExpanded ? "◀" : "▶"}
    </button>
  </header>

  {#if isExpanded}
    <section class="panel">
      <h3>Explorer</h3>
      <p>Próximamente...</p>
    </section>

    <section class="panel">
      <h3>Terminal</h3>
      <p class="status">{termMsg}</p>
      <button on:click={spawnTerm}>Reabrir WezTerm</button>
    </section>

    <section class="panel">
      <h3>Paneles</h3>
      <button on:click={listPanes}>Listar Paneles</button>
      {#if paneList}
        <pre class="pane-list">{paneList}</pre>
      {/if}
    </section>

    <section class="panel">
      <h3>Enviar Comando</h3>
      <input type="text" bind:value={commandText} placeholder="Comando..." />
      <button on:click={sendCommand}>Enviar a WezTerm</button>
    </section>
  {/if}
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: system-ui, -apple-system, sans-serif;
  }

  .sidebar {
    width: 100%;
    height: 100vh;
    background: #1e1e1e;
    color: #cccccc;
    display: flex;
    flex-direction: column;
    padding: 16px;
    box-sizing: border-box;
    transition: all 0.3s ease;
  }

  .sidebar.collapsed {
    padding: 8px;
    align-items: center;
  }

  header {
    border-bottom: 1px solid #333;
    padding-bottom: 12px;
    margin-bottom: 16px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .collapsed header {
    border-bottom: none;
    padding-bottom: 0;
    justify-content: center;
  }

  header h2 {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
    color: #fff;
  }

  .toggle-btn {
    background: #333;
    color: #ccc;
    border: none;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    margin: 0;
    min-width: 28px;
  }

  .toggle-btn:hover {
    background: #444;
    color: #fff;
  }

  .panel {
    margin-bottom: 20px;
  }

  .panel h3 {
    margin: 0 0 8px 0;
    font-size: 13px;
    text-transform: uppercase;
    color: #858585;
    font-weight: 500;
  }

  .panel p {
    margin: 0;
    font-size: 13px;
    color: #858585;
  }

  .status {
    color: #4ec9b0 !important;
    font-style: italic;
    margin-bottom: 8px !important;
  }

  button {
    background: #0e639c;
    color: white;
    border: none;
    padding: 6px 12px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    margin-top: 8px;
  }

  button:hover {
    background: #1177bb;
  }

  input[type="text"] {
    background: #3c3c3c;
    color: #ccc;
    border: 1px solid #555;
    padding: 6px 10px;
    border-radius: 4px;
    font-size: 12px;
    width: 100%;
    box-sizing: border-box;
    margin-bottom: 8px;
    font-family: monospace;
  }

  input[type="text"]:focus {
    outline: none;
    border-color: #0e639c;
  }

  .pane-list {
    background: #252526;
    padding: 8px;
    border-radius: 4px;
    font-size: 11px;
    font-family: monospace;
    color: #ccc;
    max-height: 150px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin-top: 8px;
  }
</style>
