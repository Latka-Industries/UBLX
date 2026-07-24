#!/usr/bin/env bash
# Build CSR WASM + Tailwind CSS into ./dist for panza StaticMount
# (Dir via UBLX_WEB_DIST, or Embedded).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CRATE="$(cd "$(dirname "$0")" && pwd)"
OUT="$CRATE/dist"

cd "$ROOT"
cargo build -p ublx-web --target wasm32-unknown-unknown --release
mkdir -p "$OUT"
wasm-bindgen \
  --target web \
  --out-dir "$OUT" \
  --out-name ublx_web \
  "$ROOT/target/wasm32-unknown-unknown/release/ublx_web.wasm"
cp "$CRATE/index.html" "$CRATE/styles.css" "$OUT/"
# Replace section CSS (do not `cp -R` into an existing `dist/styles/` — that nests
# as `dist/styles/styles/` and leaves stale `help.css` / etc.).
rm -rf "$OUT/styles"
cp -R "$CRATE/styles" "$OUT/styles"

# Built Tailwind utilities (no CDN). Needs Node/npm.
if ! command -v npm >/dev/null 2>&1; then
  echo "error: npm required to build Tailwind CSS (install Node.js)" >&2
  exit 1
fi
(
  cd "$CRATE"
  if [[ ! -d node_modules/tailwindcss ]]; then
    if [[ -f package-lock.json ]]; then
      npm ci
    else
      npm install
    fi
  fi
  npx --no-install @tailwindcss/cli \
    -i ./styles/tailwind-input.css \
    -o "$OUT/tailwind.css" \
    --minify
)

# Sync into package-local path for rust-embed + crates.io (`cargo install --features ui`).
EMBED_OUT="$ROOT/assets/web-ui"
rm -rf "$EMBED_OUT"
mkdir -p "$(dirname "$EMBED_OUT")"
cp -a "$OUT" "$EMBED_OUT"

echo "built $OUT"
echo "synced $EMBED_OUT"
echo "  Dir (dev):   UBLX_WEB_DIST=$OUT cargo run -p ublx --features ui -- serve . --open"
echo "  Embedded:    cargo build -p ublx --features ui   # then run without UBLX_WEB_DIST"
