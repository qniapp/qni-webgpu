#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_DIR="$APP_DIR/crates/circuit-library-model"
MODEL_TARGET_DIR="$APP_DIR/target/circuit-library-model"

cargo fmt --manifest-path "$MODEL_DIR/Cargo.toml" --check
cargo fmt --manifest-path "$APP_DIR/Cargo.toml" --check
rustfmt --edition 2021 --check "$APP_DIR/scripts/check_capacity_errors.rs"
CARGO_TARGET_DIR="$MODEL_TARGET_DIR" cargo clippy --locked --manifest-path "$MODEL_DIR/Cargo.toml" --all-targets -- -D warnings
CARGO_TARGET_DIR="$MODEL_TARGET_DIR" cargo test --locked --manifest-path "$MODEL_DIR/Cargo.toml"
cargo clippy --locked --manifest-path "$APP_DIR/Cargo.toml" --target wasm32-unknown-unknown --tests -- -D warnings
cargo test --locked --manifest-path "$APP_DIR/Cargo.toml" --target wasm32-unknown-unknown --no-run
mkdir -p "$APP_DIR/target/check-scripts"
rustc --edition=2021 "$APP_DIR/scripts/check_capacity_errors.rs" -o "$APP_DIR/target/check-scripts/check_capacity_errors"
"$APP_DIR/target/check-scripts/check_capacity_errors"
(
  cd "$APP_DIR"
  cargo insta pending-snapshots
)
