#!/usr/bin/env bash
set -euo pipefail

web_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
crate_dir="$web_dir/verification/kani"
version=$(cargo kani --version)
if ! grep -Fxq 'Kani Rust Verifier 0.68.0 (cargo plugin)' <<< "$version"; then
  echo 'Kani 0.68.0 が必要です。docs/formal-verification.md を参照してください。' >&2
  exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
log_dir=${KANI_LOG_DIR:-$scratch/logs}
mkdir -p -- "$log_dir"

# 計画の証明範囲がローカル実行の上限と一致することを確認する。
python3 - "$web_dir" <<'PY'
from pathlib import Path
import sys

web = Path(sys.argv[1])
constants = (web / "src/constants.rs").read_text().splitlines()
if constants.count("pub(crate) const LOCAL_MAX_QUBITS: usize = 16;") != 1:
    sys.exit("ローカル量子ビット数の上限が変更されています。証明範囲を見直してください。")
PY

# 実ソースが全件成功しなければ、故障版の検証には進まない。
cargo kani --manifest-path "$crate_dir/Cargo.toml" 2>&1 | tee "$log_dir/pass.txt"
grep -Fq 'VERIFICATION:- SUCCESSFUL' "$log_dir/pass.txt"
grep -Fxq 'Complete - 7 successfully verified harnesses, 0 failures, 7 total.' "$log_dir/pass.txt"

# 毎回実ソースから復元し、前の故障注入が次の証明へ混入しないようにする。
reset_mutant() {
  mkdir -p "$scratch/web/verification/kani" "$scratch/web/src/gates" \
    "$scratch/web/src/simulation_plan" "$scratch/web/src/gpu"
  cp "$crate_dir/Cargo.toml" "$crate_dir/Cargo.lock" "$crate_dir/lib.rs" \
    "$scratch/web/verification/kani/"
  cp "$web_dir/src/gates/controls.rs" "$scratch/web/src/gates/"
  cp "$web_dir/src/qubit_bit.rs" "$scratch/web/src/"
  cp "$web_dir/src/gpu/arithmetic_index.rs" "$scratch/web/src/gpu/"
  cp "$web_dir/src/simulation_plan/arithmetic_field.rs" "$scratch/web/src/simulation_plan/"
}

check_mutant() {
  local label=$1 relative=$2 old=$3 replacement=$4 harness=$5 assertion=$6 log_name=$7
  reset_mutant
  echo "=== 故障版: $label ==="
  python3 - "$scratch/web/$relative" "$old" "$replacement" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
source = path.read_text()
old, replacement = sys.argv[2:]
if source.count(old) != 1:
    sys.exit(f"故障注入対象が一意に見つかりません: {path}: {old}")
path.write_text(source.replace(old, replacement))
PY
  if cargo kani --manifest-path "$scratch/web/verification/kani/Cargo.toml" \
    --harness "$harness" 2>&1 | tee "$log_dir/$log_name"; then
    echo '故障版が成功しました。検証条件を確認してください。' >&2
    exit 1
  fi
  grep -Fxq 'VERIFICATION:- FAILED' "$log_dir/$log_name"
  grep -Fxq "Failed Checks: $assertion" "$log_dir/$log_name"
  grep -Fxq 'Complete - 0 successfully verified harnesses, 1 failures, 1 total.' "$log_dir/$log_name"
}

check_mutant '対象範囲の内外を反転する制御マスク' \
  src/gates/controls.rs 'let keep = !span_mask;' 'let keep = span_mask;' \
  restricted_outside_removes_only_inside_controls \
  'restricted_outside clears inside bits and preserves outside mask/value bits' mutant.txt

check_mutant '実 Rust の減算量を u32::MAX - 1 へ変更' \
  src/simulation_plan/arithmetic_field.rs \
  '            u32::MAX' '            u32::MAX - 1' \
  arithmetic_plan_preconditions \
  'arithmetic plan has exact mask fit disjoint controls and correct amount' mutant-plan-amount.txt

check_mutant '実コンストラクタの対象マスクを 1 だけずらす' \
  src/simulation_plan/arithmetic_field.rs \
  'mask: (1u32 << span) - 1,' 'mask: (1u32 << span) - 2,' \
  arithmetic_plan_preconditions \
  'arithmetic plan has exact mask fit disjoint controls and correct amount' mutant-plan.txt

check_mutant '実 Rust の制御除去で対象範囲のシフトを省略' \
  src/simulation_plan/arithmetic_field.rs \
  'controls.restricted_outside(self.mask << self.shift)' \
  'controls.restricted_outside(self.mask)' \
  arithmetic_plan_preconditions \
  'arithmetic plan has exact mask fit disjoint controls and correct amount' mutant-plan-controls.txt

check_mutant '対象フィールドをゼロに潰す' \
  src/gpu/arithmetic_index.rs \
  'let previous = field.wrapping_sub(amount) & target_mask;' \
  'let previous = field.wrapping_sub(amount) & 0u32;' \
  arithmetic_permutation 'arithmetic source lookup is injective' mutant-permutation.txt

check_mutant '範囲外ビットを消す' \
  src/gpu/arithmetic_index.rs '(i & !mask)' '0u32' \
  arithmetic_outside_bits 'arithmetic preserves outside bits' mutant-outside.txt

check_mutant '加減算量を常に 1 にする' \
  src/gpu/arithmetic_index.rs 'field.wrapping_sub(amount)' 'field.wrapping_sub(1)' \
  arithmetic_inverse 'arithmetic opposite amounts are inverse' mutant-inverse.txt

check_mutant '加減算方向を反転する' \
  src/gpu/arithmetic_index.rs 'field.wrapping_sub(amount)' 'field.wrapping_add(amount)' \
  arithmetic_direction 'arithmetic source plus amount equals destination field' mutant-direction.txt

echo '正常版の全 7 件は成功し、故障版の全 8 件はそれぞれ対応する 1 件だけ失敗しました。'
