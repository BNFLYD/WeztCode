// Glifos de Nerd Fonts por nombre de Iconify ("nf-dev-javascript", ...).
// Los codepoints salen de nerd-fonts/glyphnames.json (v3.5.1).
//
// Se renderizan como TEXTO (con una Nerd Font) en vez de como SVG de
// Iconify: es lo que ya usa la terminal, pesa menos y hereda el color
// del texto donde está.
export const NF_GLYPHS = {
  // devicons
  "nf-dev-rust": "\u{e7a8}",
  "nf-dev-javascript": "\u{e781}",
  "nf-dev-typescript": "\u{e8ca}",
  "nf-dev-svelte": "\u{e8b7}",
  "nf-dev-lua": "\u{e826}",
  "nf-dev-html5": "\u{e736}",
  "nf-dev-markdown": "\u{e73e}",
  "nf-dev-json": "\u{e80b}",
  "nf-dev-python": "\u{e73c}",
  "nf-dev-yaml": "\u{e8eb}",
  "nf-dev-git": "\u{e702}",
  // codicons
  "nf-cod-folder": "\u{ea83}",
  "nf-cod-folder_opened": "\u{eaf7}",
  "nf-cod-arrow_left": "\u{ea9b}",
  // material design
  "nf-md-image": "\u{f02e9}",
  "nf-md-movie": "\u{f0381}",
  "nf-md-cog": "\u{f0493}",
  // font awesome
  "nf-fa-file": "\u{f15b}",
  "nf-fa-scissors": "\u{f0c4}",
};

// Nombre de Iconify ("nf-...") → glifo. "" si no está en el mapa.
export function glyph_for(name) {
  return NF_GLYPHS[name] ?? "";
}
