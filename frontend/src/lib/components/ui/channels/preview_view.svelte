<script>
  // Vista de preview: la imagen del archivo bajo el cursor del explorer.
  // Cada imagen es una vista con key propia (preview:{path}): pasar de
  // una imagen a otra es un cambio de estado (estática → vista nueva).
  let { image_path = "", name = "" } = $props();
</script>

<img
  src="/api/fs/image?path={encodeURIComponent(image_path)}"
  class="absolute inset-0 w-full h-full object-contain z-10"
  alt={name}
  onerror={(e) => console.error("[preview] img error:", e.target.src)}
/>
