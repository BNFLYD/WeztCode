<script>
  // Visual del agente activo en el monitor, según su estado. El URL
  // key-ea por agent+state: el cache del browser guarda el asset
  // correcto de cada agente (switch → URL distinto → imagen nueva, y
  // volver al agente anterior pega al cache: 0 requests). El ciclo de
  // vida lo maneja el canal del chat (chat_channel.js): los estados del
  // MISMO agente (standby|talking|thinking|confused) son estados vivos
  // de la misma vista (swap en caliente, sin estática); cambiar de
  // agente es un update (estática → visual nuevo).
  let { state = "standby", agent_name = "" } = $props();
</script>

<img
  src="/api/chat/asset?agent={encodeURIComponent(agent_name)}&state={state}"
  class="absolute inset-0 w-full h-full object-contain"
  alt={agent_name}
  draggable="false"
  onerror={(e) => console.error("[chat-channel] asset error:", e.target.src)}
/>
