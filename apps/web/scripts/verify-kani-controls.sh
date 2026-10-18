#!/usr/bin/env bash
set -euo pipefail

web_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
crate_dir="$web_dir/verification/kani-controls"
version=$(cargo kani --version)
if ! grep -Fxq 'Kani Rust Verifier 0.68.0 (cargo plugin)' <<< "$version"; then
  echo 'Kani 0.68.0 が必要です。docs/formal-verification.md を参照してください。' >&2
  exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
log_dir=${KANI_CONTROLS_LOG_DIR:-$scratch/logs}
mkdir -p -- "$log_dir"

# 実ソースが成功しなければ、故障版の検証には進まない。
cargo kani --manifest-path "$crate_dir/Cargo.toml" 2>&1 | tee "$log_dir/pass.txt"
grep -Fq 'VERIFICATION:- SUCCESSFUL' "$log_dir/pass.txt"
grep -Eq 'Complete - 1 successfully verified harnesses, 0 failures' "$log_dir/pass.txt"

# 相対取り込みを維持した隔離コピーだけを書き換える。
mkdir -p "$scratch/web/verification/kani-controls" "$scratch/web/src/gates"
cp "$crate_dir/Cargo.toml" "$crate_dir/Cargo.lock" "$crate_dir/lib.rs" \
  "$scratch/web/verification/kani-controls/"
cp "$web_dir/src/gates/controls.rs" "$scratch/web/src/gates/"
cp "$web_dir/src/qubit_bit.rs" "$scratch/web/src/"
mutant="$scratch/web/src/gates/controls.rs"
if [[ $(grep -Fc 'let keep = !span_mask;' "$mutant") != 1 ]]; then
  echo '故障注入対象が一意に見つかりません。検証を中止します。' >&2
  exit 1
fi
sed -i 's/let keep = !span_mask;/let keep = span_mask;/' "$mutant"
if cargo kani --manifest-path "$scratch/web/verification/kani-controls/Cargo.toml" \
  2>&1 | tee "$log_dir/mutant.txt"; then
  echo '故障版が成功しました。検証条件を確認してください。' >&2
  exit 1
fi
grep -Fq 'VERIFICATION:- FAILED' "$log_dir/mutant.txt"
grep -Fxq 'Failed Checks: restricted_outside clears inside bits and preserves outside mask/value bits' \
  "$log_dir/mutant.txt"
grep -Eq 'Complete - 0 successfully verified harnesses, 1 failures' "$log_dir/mutant.txt"
echo '正常版は成功し、故障版は失敗しました。'
