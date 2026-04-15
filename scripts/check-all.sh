#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Web: Playwright preflight (browser resolution)"
pnpm -C "$ROOT_DIR/apps/egui-web" run test:preflight

echo "==> Web: Playwright (egui)"
pnpm -C "$ROOT_DIR/apps/egui-web" exec playwright test

echo "==> MCP: pnpm check"
pnpm -C "$ROOT_DIR/apps/mcp-qni" check

echo "==> TUI: make check"
make -C "$ROOT_DIR" check
