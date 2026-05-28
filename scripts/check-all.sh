#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Web: Rust fmt/clippy/wasm no-run/insta"
"$ROOT_DIR/apps/web/scripts/check-rust.sh"

echo "==> Web: production trunk build"
(
  cd "$ROOT_DIR/apps/web"
  env -u NO_COLOR TRUNK_COLOR=never trunk build --release --public-url ./
)

echo "==> Web: Playwright preflight (browser resolution)"
pnpm -C "$ROOT_DIR/apps/web" run test:preflight

echo "==> Web: Cucumber BDD"
pnpm -C "$ROOT_DIR/apps/web" run test:bdd

echo "==> Web: Playwright legacy"
pnpm -C "$ROOT_DIR/apps/web" run test:pw-legacy

echo "==> Deployment config tests"
node --test "$ROOT_DIR/test-node"/*.test.cjs

echo "==> Qiskit backend editable install smoke"
qiskit_backend_venv="$(mktemp -d)"
python3 -m venv "$qiskit_backend_venv"
"$qiskit_backend_venv/bin/python" -m pip install --upgrade pip >/dev/null
"$qiskit_backend_venv/bin/python" -m pip install --no-deps -e "$ROOT_DIR/apps/qiskit-backend"
rm -rf "$qiskit_backend_venv"

echo "==> Qiskit backend tests"
PYTHONPATH="$ROOT_DIR/apps/qiskit-backend/src" python3 -m unittest discover "$ROOT_DIR/apps/qiskit-backend/tests"

echo "==> TUI: make check"
make -C "$ROOT_DIR" check
