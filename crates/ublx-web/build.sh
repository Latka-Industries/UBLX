#!/usr/bin/env bash
# Build CSR WASM into ./dist for panza StaticMount::Dir (feature `ui`).
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
cp -R "$CRATE/styles" "$OUT/styles"
echo "built $OUT (open via: cargo run -p ublx --features ui -- serve . --open)"
