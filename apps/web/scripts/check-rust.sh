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
rustfmt --edition 2021 --check "$APP_DIR/scripts/check_capacity_errors.rs"
cargo clippy --locked --manifest-path "$APP_DIR/Cargo.toml" --target wasm32-unknown-unknown --tests -- -D warnings
cargo test --locked --manifest-path "$APP_DIR/Cargo.toml" --target wasm32-unknown-unknown --no-run
# Run Web crate unit tests on the host too. The app's normal build target is
# wasm32, but host lib tests are useful for pure Rust modules such as URL
# parsing and value objects; enabling eframe's x11 feature gives winit a native
# platform for the test build without changing the production wasm path.
RUSTFLAGS="-Awarnings" cargo test --locked --manifest-path "$APP_DIR/Cargo.toml" --lib --features eframe/x11
mkdir -p "$APP_DIR/target/check-scripts"
rustc --edition=2021 "$APP_DIR/scripts/check_capacity_errors.rs" -o "$APP_DIR/target/check-scripts/check_capacity_errors"
"$APP_DIR/target/check-scripts/check_capacity_errors"
(
  cd "$APP_DIR"
  cargo insta pending-snapshots
)
