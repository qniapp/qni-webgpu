#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

DUMP_OUT=${TUI_DUMP_OUT:-/tmp/tui-latest.dump}
SNAPSHOT_PNG_OUT=${TUI_SNAPSHOT_PNG_OUT:-/tmp/tui-latest.png}
TERMINAL_PNG_OUT=${TUI_TERMINAL_PNG_OUT:-/tmp/tui-terminal-screenshot.png}

(cd "$ROOT_DIR" && cargo run --bin snapshot_dump -- --out "$DUMP_OUT")
"$ROOT_DIR/scripts/render_snapshot_png.py" "$DUMP_OUT" "$SNAPSHOT_PNG_OUT"

TUI_SCREENSHOT_OUT="$TERMINAL_PNG_OUT" \
  "$ROOT_DIR/scripts/tui_terminal_screenshot.sh" > /dev/null

echo "dump: $DUMP_OUT"
echo "snapshot_png: $SNAPSHOT_PNG_OUT"
echo "terminal_png: $TERMINAL_PNG_OUT"
