#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Web: Rust fmt/clippy/wasm no-run/insta"
"$ROOT_DIR/apps/web/scripts/check-rust.sh"

echo "==> Web: Playwright preflight (browser resolution)"
pnpm -C "$ROOT_DIR/apps/web" run test:preflight

echo "==> Web: Cucumber BDD"
pnpm -C "$ROOT_DIR/apps/web" run test:bdd

echo "==> Web: Playwright legacy"
pnpm -C "$ROOT_DIR/apps/web" run test:pw-legacy

echo "==> Qiskit backend: unit tests"
PYTHONPATH="$ROOT_DIR/apps/qiskit-backend/src" python3 -m unittest discover "$ROOT_DIR/apps/qiskit-backend/tests"

echo "==> MCP: pnpm check"
pnpm -C "$ROOT_DIR/apps/mcp-qni" check

echo "==> TUI: make check"
make -C "$ROOT_DIR" check
