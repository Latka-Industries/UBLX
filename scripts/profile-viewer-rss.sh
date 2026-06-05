#!/usr/bin/env bash
# Manual RSS regression check for viewer preview switching ([#28]).
#
# Usage:
#   ./scripts/profile-viewer-rss.sh [path/to/indexed/dir]
#
# macOS: prints max RSS via /usr/bin/time -l after a short warm-up run.
# Linux: uses /usr/bin/time -v when available (heaptrack/valgrind are optional follow-ups).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TARGET_DIR="${1:-.}"
BIN="${CARGO_TARGET_DIR:-target}/release/ublx"

echo "Building release binary..."
cargo build --release --quiet

echo ""
echo "=== Manual RSS regression (#28) ==="
echo "1. Index once, then hop between large previews in the TUI (image, PDF, markdown, CSV)."
echo "2. Watch RSS in Activity Monitor (macOS) or htop — it should plateau, not grow per switch."
echo "3. Optional profilers: Instruments Allocations, heaptrack, DHAT."
echo ""
echo "Quick subprocess smoke (not a substitute for TUI profiling):"

if [[ "$(uname -s)" == "Darwin" ]]; then
  /usr/bin/time -l "$BIN" "$TARGET_DIR" --help >/dev/null 2>&1 || true
  echo ""
  echo "See 'maximum resident set size' above for baseline CLI RSS."
else
  if command -v /usr/bin/time >/dev/null 2>&1; then
    /usr/bin/time -v "$BIN" "$TARGET_DIR" --help 2>&1 | grep -E 'Maximum resident|Elapsed' || true
  fi
  echo ""
  echo "For heap growth while switching previews, run heaptrack against an interactive session."
fi

echo ""
echo "Automated guards: cargo test --test performance"
