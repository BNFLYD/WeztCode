#!/usr/bin/env bash
# Build del bundle ejecutable de WeztCode (app local de prueba).
#
# El binario sirve el frontend y lee user_props.lua con rutas RELATIVAS al
# directorio donde se ejecuta, así que el bundle reproduce ese layout:
#
#   weztcode-bundle/
#   ├── weztcode            ← binario release
#   ├── user_props.lua      ← config (editable)
#   └── frontend/dist/      ← UI compilada
#
# Uso:      ./build-bundle.sh
# Ejecutar:  cd weztcode-bundle && ./weztcode
#
# Nota: la app hardcodea el puerto 8765 — no correr dos instancias juntas.

set -euo pipefail
cd "$(dirname "$0")"

OUT="weztcode-bundle"

echo "==> Cargo release build..."
cargo build --release

echo "==> Frontend build..."
(cd frontend && pnpm build)

echo "==> Ensamblando bundle..."
rm -rf "$OUT"
mkdir -p "$OUT/frontend"
cp target/release/weztcode "$OUT/weztcode"
cp user_props.lua "$OUT/user_props.lua"
cp -r frontend/dist "$OUT/frontend/"

echo "==> Bundle listo:"
ls -la "$OUT"
du -sh "$OUT"
echo ""
echo "    Ejecutá con:  cd $OUT && ./weztcode"
