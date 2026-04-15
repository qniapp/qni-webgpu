#!/usr/bin/env bash
set -euo pipefail

PORT="${QNI_EGUI_WEB_PORT:-4174}"
URL="${QNI_EGUI_WEB_URL:-http://127.0.0.1:${PORT}/}"
PROFILE_DIR="${QNI_EGUI_WEB_PROFILE_DIR:-/tmp/qni-webgpu-chrome-profile}"
BROWSER_BIN="${QNI_EGUI_WEB_BROWSER:-}"

find_browser() {
  if [[ -n "$BROWSER_BIN" ]]; then
    printf '%s\n' "$BROWSER_BIN"
    return 0
  fi

  local candidate
  for candidate in google-chrome-stable google-chrome chromium chromium-browser chrome; do
    if command -v "$candidate" >/dev/null 2>&1; then
      command -v "$candidate"
      return 0
    fi
  done

  echo "No Chrome/Chromium browser found. Set QNI_EGUI_WEB_BROWSER to override." >&2
  return 1
}

wait_for_server() {
  python - "$URL" <<'PY'
import sys, urllib.request
url = sys.argv[1]
with urllib.request.urlopen(url, timeout=2) as response:
    if response.status != 200:
        raise SystemExit(1)
PY
}

browser="$(find_browser)"
mkdir -p "$PROFILE_DIR"

if ! wait_for_server; then
  cat >&2 <<EOF
qni-egui-web server is not reachable at:
  $URL

Start it first:
  cd apps/egui-web
  trunk serve --address 127.0.0.1 --port $PORT
EOF
  exit 1
fi

exec "$browser" \
  --user-data-dir="$PROFILE_DIR" \
  --new-window \
  --no-first-run \
  --no-default-browser-check \
  --ozone-platform=x11 \
  --enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan \
  --enable-unsafe-webgpu \
  --ignore-gpu-blocklist \
  "$URL"
