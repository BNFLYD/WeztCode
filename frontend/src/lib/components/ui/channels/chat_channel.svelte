<script>
  // Channel del chat: visual del agente activo en el monitor, según su estado.
  // El URL key-ea por agent+state: el cache del browser guarda el asset correcto
  // de cada agente (switch → URL distinto → imagen nueva, y volver al agente
  // anterior pega al cache: 0 requests).
  // El ciclo de vida lo maneja +page: los canales viven solo dentro de su
  // sección (handle_section_change limpia active_channel al salir).
  export let state = "standby"; // standby | talking | thinking | confused
  export let agent_name = "";
</script>

<img
  src="/api/chat/asset?agent={encodeURIComponent(agent_name)}&state={state}"
  class="absolute inset-0 w-full h-full object-contain"
  alt={agent_name}
  draggable="false"
  onerror={(e) => console.error("[chat-channel] asset error:", e.target.src)}
/>
