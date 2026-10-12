<script>
  // Channel del chat: visual del agente activo en el monitor, según su estado.
  // El URL key-ea por agent+state: el cache del browser guarda el asset correcto
  // de cada agente (switch → URL distinto → imagen nueva, y volver al agente
  // anterior pega al cache: 0 requests).
  export let state = "standby"; // standby | talking | thinking | confused
  export let agent_name = "";

  // Auto-cierre al salir de la sección chat (patrón del explorer_channel):
  // ChatSection se DESMONTA al cambiar de sección, así que su $effect muere con
  // ella — el canal tiene que detectar el cambio solo. El Monitor (siempre
  // montado) le sigue pasando active_section a cualquier channel.
  export let active_section = "chat";
  export let on_close = () => {};

  $: if (active_section !== "chat") {
    on_close();
  }
</script>

<img
  src="/api/chat/asset?agent={encodeURIComponent(agent_name)}&state={state}"
  class="absolute inset-0 w-full h-full object-contain"
  alt={agent_name}
  draggable="false"
  onerror={(e) => console.error("[chat-channel] asset error:", e.target.src)}
/>
