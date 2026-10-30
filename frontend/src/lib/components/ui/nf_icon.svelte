<script>
  import Icon from "@iconify/svelte";
  import { glyph_for } from "$lib/nerd_glyphs.js";

  // Icono híbrido:
  //   - nombre "nf-*" → glifo de Nerd Font (texto con la fuente NF);
  //   - cualquier otro → SVG de Iconify (como siempre).
  // El tamaño sale del font-size del contexto (1em), no de clases w-*/h-*.
  let { icon = "", class: klass = "" } = $props();
</script>

{#if icon.startsWith("nf-")}
  <span class={`nf-glyph ${klass}`}>{glyph_for(icon)}</span>
{:else}
  <Icon {icon} class={klass} />
{/if}
