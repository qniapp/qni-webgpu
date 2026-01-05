#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Web: pnpm check"
pnpm -C "$ROOT_DIR/apps/web" check

echo "==> MCP: pnpm check"
pnpm -C "$ROOT_DIR/apps/mcp-qni" check

echo "==> TUI: make check"
make -C "$ROOT_DIR" check
