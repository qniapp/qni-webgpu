#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

WIDTH="${TUI_WIDTH:-80}"
HEIGHT="${TUI_HEIGHT:-30}"
OUT_PATH="${TUI_SCREENSHOT_OUT:-/tmp/tui-terminal-screenshot.png}"
FONT_FAMILY="${TUI_FONT_FAMILY:-Caskaydia Mono Nerd Font}"
BG_COLOR="${TUI_BACKGROUND_COLOR:-black}"
MARGIN="${TUI_SCREENSHOT_MARGIN:-0}"

cd "$ROOT_DIR"

cargo run --quiet --bin snapshot_ansi -- --width "$WIDTH" --height "$HEIGHT" \
  | npx --yes terminal-screenshot \
    --output "$OUT_PATH" \
    --font-family "$FONT_FAMILY" \
    --background-color "$BG_COLOR" \
    --margin "$MARGIN"

echo "$OUT_PATH"
