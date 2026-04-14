#!/usr/bin/env bash
set -euo pipefail

find_config() {
  local dir
  dir="$PWD"
  while true; do
    if [[ -f "$dir/.playwright-mcp/config.json" ]]; then
      printf '%s' "$dir/.playwright-mcp/config.json"
      return 0
    fi
    if [[ "$dir" == "/" ]]; then
      return 1
    fi
    dir="$(dirname "$dir")"
  done
}

run_mcp() {
  if command -v xvfb-run >/dev/null 2>&1; then
    exec xvfb-run -a -s "-screen 0 1920x1080x24" npx @playwright/mcp@latest --isolated "$@"
  fi
  exec npx @playwright/mcp@latest --isolated "$@"
}

config_path=""
if config_path="$(find_config)"; then
  run_mcp --config "$config_path" "$@"
fi

run_mcp "$@"
