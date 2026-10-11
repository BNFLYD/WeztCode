<script>
  // Channel del chat: visual del agente activo en el monitor, según su estado.
  // Los bytes los sirve el backend (/api/chat/asset) resolviendo el archivo
  // desde la carpeta `assets:` del .md del agente; el browser cachea por URL
  // de estado, así que los swaps entre talking/thinking/etc. son instantáneos.
  export let state = "standby"; // standby | talking | thinking | confused
  export let agent_name = "";
</script>

<img
  src="/api/chat/asset?state={state}"
  class="absolute inset-0 w-full h-full object-contain"
  alt={agent_name}
  draggable="false"
  onerror={(e) => console.error("[chat-channel] asset error:", e.target.src)}
/>
