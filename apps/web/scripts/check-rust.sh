#!/usr/bin/env bash
set -euo pipefail

APP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODEL_CRATES=(
  "$APP_DIR/crates/circuit-library-model"
  "$APP_DIR/crates/external-gpu-model"
)

for model_dir in "${MODEL_CRATES[@]}"; do
  model_name="$(basename "$model_dir")"
  model_target_dir="$APP_DIR/target/$model_name"
  cargo fmt --manifest-path "$model_dir/Cargo.toml" --check
  CARGO_TARGET_DIR="$model_target_dir" cargo clippy --locked --manifest-path "$model_dir/Cargo.toml" --all-targets -- -D warnings
  CARGO_TARGET_DIR="$model_target_dir" cargo test --locked --manifest-path "$model_dir/Cargo.toml"
done

cargo fmt --manifest-path "$APP_DIR/Cargo.toml" --check
cargo clippy --locked --manifest-path "$APP_DIR/Cargo.toml" --target wasm32-unknown-unknown --tests -- -D warnings
cargo test --locked --manifest-path "$APP_DIR/Cargo.toml" --target wasm32-unknown-unknown --no-run
RUSTFLAGS="-D warnings" cargo test --locked --manifest-path "$APP_DIR/Cargo.toml" --lib
cargo test --locked --manifest-path "$APP_DIR/Cargo.toml" --test arithmetic_codegen
(
  cd "$APP_DIR"
  cargo insta pending-snapshots
)
