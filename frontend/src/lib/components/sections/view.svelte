<script>
  const notifications = [
    { id: 1, title: "Notificacion 1", message: "Mensaje de notificacion importante" },
    { id: 2, title: "Notificacion 2", message: "Mensaje de notificacion importante" },
    { id: 3, title: "Notificacion 3", message: "Mensaje de notificacion importante" },
  ];
</script>

<div class="flex flex-col gap-2.5 py-4">
  {#each notifications as notification}
    <div
      class="bg-back rounded-xl p-3.5 space-y-1 hover:bg-accent-detail/10 transition-colors cursor-pointer"
    >
      <p class="text-xs font-bold text-print">{notification.title}</p>
      <p class="text-xs text-print-contrast/70">{notification.message}</p>
    </div>
  {/each}
</div>

<style>
  button:focus-visible {
    outline: none;
  }
</style>