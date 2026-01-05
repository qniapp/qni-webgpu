#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for tool in cargo-audit cargo-deny; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "Missing $tool. Run: cargo install cargo-audit cargo-deny" >&2
    exit 1
  fi
done

cd "$ROOT_DIR/apps/tui"
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo audit
cargo deny check --config "$ROOT_DIR/deny.toml"
