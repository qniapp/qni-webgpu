# web `constants.rs` 抽出設計（thin-entry 次パス）

## 背景
- `apps/web/src/lib.rs` は `shared.rs` pass 完了後、91 LOC まで縮小した。
- 現在の `lib.rs` に残る主責務は次の 4 系統である。
  - 共有定数
  - `PALETTE_GATES`
  - wasm export（`start`, `read_state_vector`）
  - module wiring
- ここまでの thin-entry 系 pass では、`gates.rs` / `colors.rs` / `shared.rs` / `app.rs` / `render.rs` / `gpu.rs` / `layout.rs` へ責務を段階的に逃がしており、残る自然な cohesive block は **定数群** である。
- ユーザーは次パスとして **A: constants pass** を選択し、さらに **A1: `constants.rs` 1 ファイルにまとめる**方針を選択した。

## 目的
- `lib.rs` に残っている共有定数と `PALETTE_GATES` を `apps/web/src/constants.rs` へ安全に抽出し、crate root をさらに thin-entry 化する。
- 定数利用側を `crate::constants::...` の直接参照に揃え、root alias 依存を取り除く。
- 挙動変更なしの code motion に徹し、既存 UI / WebGPU / drag / wasm export の振る舞いを一切変えない。

## 非目的
- wasm export（`start`, `read_state_vector`）の移動や公開面変更。
- `gates.rs` / `colors.rs` / `shared.rs` / `app.rs` / `render.rs` / `gpu.rs` / `layout.rs` の責務再編。
- 定数の責務別細分化（`layout_constants.rs` / `state_constants.rs` など）。
- `gpu.rs` の shader 定数や `icons.rs` の `VIEWBOX` の移動。
- 定数名変更、値変更、レイアウト調整、描画調整、最適化。
- root re-export の追加。
- tests の変更。
- repo-wide CI 相当 aggregate check の導入。

## 採用方針
この pass では **`constants.rs` を 1 ファイル追加し、`lib.rs` の定数群と `PALETTE_GATES` を pure extraction する**。

移動対象:
- `REM`
- `STATE_CIRCLE_SIZE`
- `STATE_CIRCLE_GAP`
- `STATE_CIRCLE_BOTTOM_MARGIN`
- `STATE_CIRCLE_STROKE`
- `MIN_QUBITS`
- `MAX_QUBITS`
- `MAX_STATE_COUNT`
- `LINE_Y`
- `LINE_GAP`
- `CIRCUIT_PADDING`
- `QUBIT_LABEL_WIDTH`
- `QUBIT_LABEL_GAP`
- `LINE_LEFT_OFFSET`
- `LINE_RIGHT_OFFSET`
- `GATE_SIZE`
- `SLOT_SPACING`
- `SNAP_DISTANCE`
- `DRAG_REPAINT_BASE_SECS`
- `DRAG_REPAINT_MIN_SECS`
- `DRAG_REPAINT_MAX_SECS`
- `DRAG_REPAINT_PUMP_FACTOR`
- `PALETTE_SIZE`
- `PALETTE_GAP`
- `PALETTE_ROW_Y`
- `PALETTE_GATES`

`constants.rs` では `PALETTE_GATES` の型定義に必要なため、`crate::gates::GateKind` だけを参照してよい。

この pass では次の「ついで変更」は行わない。
- 定数を複数 module に再分割する。
- `constants.rs` を public API や root re-export の受け皿にする。
- `PALETTE_GATES` を `gates.rs` に寄せる。
- wasm export を別 module に移す。
- shader 定数や icon 定数まで再編する。

## 比較した案

### 案A: `constants.rs` 1 ファイルに pure extraction（採用）
- `apps/web/src/constants.rs` に定数群と `PALETTE_GATES` を集約する。
- 利点:
  - 今回のユーザー選択 A1 と一致する。
  - pure extraction の範囲に収まり、最小リスクで `lib.rs` を薄くできる。
  - 定数 ownership が 1 箇所に固定され、今後の整理方針が明確になる。
- 欠点:
  - layout / drag / state circle / palette と異なる性質の定数が同居する。
  - 将来さらに責務別に分けたくなる余地は残る。

### 案B: layout / state / drag など責務ごとに 2〜3 module に分割する
- 利点:
  - conceptual grouping はより自然になる。
- 欠点:
  - 今回のユーザー選択 A1 に反する。
  - 移動量・import 更新・検証面が増え、pure extraction の安全性が下がる。

### 案C: 定数の一部だけを関連 module へ個別再配置する
- 例: `MAX_STATE_COUNT` を `gpu.rs` 側、layout 定数を `layout.rs` 側へ寄せる。
- 利点:
  - 各 module の self-contained 性は上がりうる。
- 欠点:
  - 今回は ownership を 1 箇所に集める目的と逆行する。
  - pure extraction ではなく責務再設計になり、スコープが膨らむ。

## この pass のモジュール境界

### 新規追加: `apps/web/src/constants.rs`
ここには、crate 内で横断利用される共有定数と palette contents を移す。

対象:
- layout / state / drag / palette / qubit 上限系の定数
- `PALETTE_GATES`

意図:
- `lib.rs` から internal implementation detail を取り除く。
- `app.rs` / `render.rs` / `layout.rs` / `gpu.rs` の参照先を明示化する。
- constants ownership を 1 箇所に固定する。

### `apps/web/src/lib.rs` に残すもの
対象:
- module wiring（`mod app; mod colors; mod constants; mod gates; mod gpu; mod icons; mod layout; mod render; mod shared;`）
- wasm export（`start`, `read_state_vector`）

意図:
- `lib.rs` を crate root / thin-entry として保つ。
- 今回は exports と root wiring だけを `lib.rs` に残し、constants 自体は root から退避する。

## 依存関係の方針
この pass の主目的は、**定数の実装位置を root から internal module へ移し、利用側を direct module path に揃えること**である。

想定する依存:
- `app.rs` → `crate::constants::{DRAG_REPAINT_BASE_SECS, DRAG_REPAINT_MAX_SECS, DRAG_REPAINT_MIN_SECS, DRAG_REPAINT_PUMP_FACTOR, GATE_SIZE, MAX_QUBITS, MIN_QUBITS, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE, SNAP_DISTANCE}`
- `render.rs` → `crate::constants::{CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, LINE_Y, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE, REM, SNAP_DISTANCE, STATE_CIRCLE_BOTTOM_MARGIN, STATE_CIRCLE_GAP, STATE_CIRCLE_SIZE, STATE_CIRCLE_STROKE}`
- `layout.rs` → `crate::constants::{GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y, SLOT_SPACING}`
- `gpu.rs` → `crate::constants::MAX_STATE_COUNT`
- `constants.rs` → `crate::gates::GateKind`
- `lib.rs` → `constants.rs`（module declaration のみ）

方針:
- `crate::REM` / `crate::GATE_SIZE` / `crate::MAX_STATE_COUNT` / `crate::PALETTE_GATES` などの old root path は残さない。
- `use crate::{...moved constants...}` の grouped root import も残さない。
- `use constants::*;` / `use crate::constants::*;` のような convenience alias も追加しない。
- root re-export（`pub use constants::...` や grouped root alias）は追加しない。
- 定数利用側は `crate::constants::...` を直接参照する。
- `colors.rs` / `shared.rs` / `gates.rs` / `icons.rs` は原則無変更とする。

## 可視性の方針
- 外部公開（`pub`）は追加しない。
- `constants.rs` に移す定数と `PALETTE_GATES` は、crate 内利用に必要な最小限として **`pub(crate) const`** にする。
- `constants.rs` 内で新しい helper や re-export は追加しない。
- root re-export（`pub use`, `pub(crate) use`, `pub(super) use`）は追加しない。
- `PALETTE_GATES` の型は `pub(crate) const PALETTE_GATES: [GateKind; 15]` とし、element 値は現状維持する。

## `start` / `read_state_vector` の扱い
wasm export の公開面は動かさない。

採用方針:
- `#[wasm_bindgen] pub async fn start(...)` は `lib.rs` に残す。
- `#[wasm_bindgen] pub async fn read_state_vector(...)` は `lib.rs` に残す。
- 関数名・シグネチャ・公開位置は維持する。

## 実装ガードレール
- 移動対象定数と `PALETTE_GATES` 本体は、module path と visibility 調整を除き原則そのまま移す。
- 値変更・命名変更・式変更はしない。
- `constants.rs` は internal module に留め、public API としない。
- `start` / `read_state_vector` は `lib.rs` に残す。
- `PALETTE_GATES` の並び順は変えない。
- tests は変更しない。
- root re-export は追加しない。
- 旧 root path 参照、grouped root import、wildcard alias は残さない。
- 変更対象ファイルは `apps/web/src/lib.rs` / `apps/web/src/constants.rs` / `apps/web/src/app.rs` / `apps/web/src/render.rs` / `apps/web/src/layout.rs` / `apps/web/src/gpu.rs` に限定する。
- `colors.rs` / `shared.rs` / `gates.rs` / `icons.rs` の不要変更はしない。

## 実装手順
1. `apps/web/src/constants.rs` を追加し、`lib.rs` に `mod constants;` を宣言する。
2. `lib.rs` から定数群と `PALETTE_GATES` を `constants.rs` へ移す。
3. `app.rs` / `render.rs` / `layout.rs` / `gpu.rs` の import / 参照を `crate::constants::...` 前提に更新する。
4. 早い段階で `cargo check --target wasm32-unknown-unknown` を実行して import / visibility 崩れを切り分ける。
5. Playwright / trunk build / diff check を再実行し、挙動差分がないことを確認する。
6. 最後に symbol move / direct module use / no-root-reexport / no-old-root-path / remains / tests unchanged / LOC を確認する。

## 受け入れ条件
- `apps/web/src/constants.rs` が追加されている。
- `lib.rs` から以下の定数と `PALETTE_GATES` が移っている。
  - `REM`
  - `STATE_CIRCLE_SIZE`
  - `STATE_CIRCLE_GAP`
  - `STATE_CIRCLE_BOTTOM_MARGIN`
  - `STATE_CIRCLE_STROKE`
  - `MIN_QUBITS`
  - `MAX_QUBITS`
  - `MAX_STATE_COUNT`
  - `LINE_Y`
  - `LINE_GAP`
  - `CIRCUIT_PADDING`
  - `QUBIT_LABEL_WIDTH`
  - `QUBIT_LABEL_GAP`
  - `LINE_LEFT_OFFSET`
  - `LINE_RIGHT_OFFSET`
  - `GATE_SIZE`
  - `SLOT_SPACING`
  - `SNAP_DISTANCE`
  - `DRAG_REPAINT_BASE_SECS`
  - `DRAG_REPAINT_MIN_SECS`
  - `DRAG_REPAINT_MAX_SECS`
  - `DRAG_REPAINT_PUMP_FACTOR`
  - `PALETTE_SIZE`
  - `PALETTE_GAP`
  - `PALETTE_ROW_Y`
  - `PALETTE_GATES`
- `lib.rs` には上記定数と `PALETTE_GATES` の定義が残っていない。
- `lib.rs` には module wiring と wasm export が残っている。
- `app.rs` / `render.rs` / `layout.rs` / `gpu.rs` は `crate::constants::...` を直接参照している。
- root re-export は追加されていない。
- `cargo check --target wasm32-unknown-unknown` が通る。
- `pnpm exec playwright test` が通る。
- `env PATH="$HOME/.cargo/bin:$PATH" trunk build` が通る。
- `git diff --check` が通る。
- `apps/web/tests` と `apps/web/test-node` は未変更である。
- `lib.rs` の LOC が shared pass 完了時の baseline **91 LOC** よりさらに減っている。

## 検証
実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && env PATH="$HOME/.cargo/bin:$PATH" trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

この pass の web 検証系には `cargo insta` ベースの snapshot suite は含まれていないため、snapshot 確認は追加しない。UI 回帰は既存 Playwright を正本とする。
また、この pass の spec では CI 相当の repo-wide aggregate check（例: `./scripts/check-all.sh`）は **スコープ外** とする。理由は、今回の変更対象が `apps/web` の局所的な code motion であり、これまでの pass と同様に wasm cargo check + Playwright + trunk build + diff check を正本の受け入れ条件とするためである。

追加で、constant move と pure extraction（値・式不変、および `constants.rs` 内での定義順維持）を機械的に確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/web/src/lib.rs').read_text()
constants = Path('apps/web/src/constants.rs').read_text()
exact_lines = [
    'pub(crate) const REM: f32 = 32.0;',
    'pub(crate) const STATE_CIRCLE_SIZE: f32 = 1.25 * REM;',
    'pub(crate) const STATE_CIRCLE_GAP: f32 = 0.5 * REM;',
    'pub(crate) const STATE_CIRCLE_BOTTOM_MARGIN: f32 = 2.0 * REM;',
    'pub(crate) const STATE_CIRCLE_STROKE: f32 = 2.0;',
    'pub(crate) const MIN_QUBITS: usize = 2;',
    'pub(crate) const MAX_QUBITS: usize = 16;',
    'pub(crate) const MAX_STATE_COUNT: usize = 1 << MAX_QUBITS;',
    'pub(crate) const LINE_Y: f32 = 6.5 * REM;',
    'pub(crate) const LINE_GAP: f32 = 1.5 * REM;',
    'pub(crate) const CIRCUIT_PADDING: f32 = 2.0 * REM; // Same as PALETTE_ROW_Y for visual consistency',
    'pub(crate) const QUBIT_LABEL_WIDTH: f32 = 3.0 * 14.0; // "qN:" at font size 14',
    'pub(crate) const QUBIT_LABEL_GAP: f32 = 0.5 * REM; // Gap between label and line (0.5rem)',
    'pub(crate) const LINE_LEFT_OFFSET: f32 = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP;',
    'pub(crate) const LINE_RIGHT_OFFSET: f32 = CIRCUIT_PADDING;',
    'pub(crate) const GATE_SIZE: f32 = 1.0 * REM;',
    'pub(crate) const SLOT_SPACING: f32 = GATE_SIZE * 1.5;',
    'pub(crate) const SNAP_DISTANCE: f32 = 0.5625 * REM;',
    'pub(crate) const DRAG_REPAINT_BASE_SECS: f64 = 0.01;',
    'pub(crate) const DRAG_REPAINT_MIN_SECS: f64 = 0.004;',
    'pub(crate) const DRAG_REPAINT_MAX_SECS: f64 = 1.0 / 30.0;',
    'pub(crate) const DRAG_REPAINT_PUMP_FACTOR: f64 = 0.1;',
    'pub(crate) const PALETTE_SIZE: f32 = GATE_SIZE;',
    'pub(crate) const PALETTE_GAP: f32 = 0.5 * REM;',
    'pub(crate) const PALETTE_ROW_Y: f32 = 2.0 * REM;',
]
positions = []
for line in exact_lines:
    old_name = re.escape(line.replace('pub(crate) ', '').split(':', 1)[0])
    if re.search(old_name + r'\s*:', lib):
        raise SystemExit(f'constant still present in lib.rs: {line}')
    pos = constants.find(line)
    if pos < 0:
        raise SystemExit(f'constant missing or changed in constants.rs: {line}')
    positions.append(pos)
if positions != sorted(positions):
    raise SystemExit('constant order changed in constants.rs')
expected_palette = '''pub(crate) const PALETTE_GATES: [GateKind; 15] = [
    GateKind::H,
    GateKind::Control,
    GateKind::X,
    GateKind::Y,
    GateKind::Z,
    GateKind::SqrtX,
    GateKind::S,
    GateKind::SDagger,
    GateKind::T,
    GateKind::TDagger,
    GateKind::Phase,
    GateKind::Rx,
    GateKind::Ry,
    GateKind::Rz,
    GateKind::Swap,
];'''
if 'const PALETTE_GATES' in lib:
    raise SystemExit('PALETTE_GATES still present in lib.rs')
if expected_palette not in constants:
    raise SystemExit('PALETTE_GATES missing or reordered in constants.rs')
print('ok')
PY
```

期待値:
- `ok`

追加で、利用側が agreed mapping どおり `crate::constants::...` を使い、grouped root import や wildcard alias が残っていないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
checks = {
    'apps/web/src/app.rs': ['PALETTE_GATES', 'GATE_SIZE', 'MAX_QUBITS'],
    'apps/web/src/render.rs': ['PALETTE_GATES', 'STATE_CIRCLE_SIZE', 'CIRCUIT_PADDING'],
    'apps/web/src/layout.rs': ['LINE_LEFT_OFFSET', 'SLOT_SPACING', 'GATE_SIZE'],
    'apps/web/src/gpu.rs': ['MAX_STATE_COUNT'],
}
moved = 'REM|STATE_CIRCLE_SIZE|STATE_CIRCLE_GAP|STATE_CIRCLE_BOTTOM_MARGIN|STATE_CIRCLE_STROKE|MIN_QUBITS|MAX_QUBITS|MAX_STATE_COUNT|LINE_Y|LINE_GAP|CIRCUIT_PADDING|QUBIT_LABEL_WIDTH|QUBIT_LABEL_GAP|LINE_LEFT_OFFSET|LINE_RIGHT_OFFSET|GATE_SIZE|SLOT_SPACING|SNAP_DISTANCE|DRAG_REPAINT_BASE_SECS|DRAG_REPAINT_MIN_SECS|DRAG_REPAINT_MAX_SECS|DRAG_REPAINT_PUMP_FACTOR|PALETTE_SIZE|PALETTE_GAP|PALETTE_ROW_Y|PALETTE_GATES'
for path, symbols in checks.items():
    text = Path(path).read_text()
    if 'crate::constants::' not in text:
        raise SystemExit(f'missing direct constants module use in {path}')
    for symbol in symbols:
        if symbol not in text:
            raise SystemExit(f'missing expected moved symbol in {path}: {symbol}')
    if re.search(r'use\s+crate::\{[^}]*\b(' + moved + r')\b', text, re.S):
        raise SystemExit(f'forbidden grouped root import in {path}')
    if re.search(r'use\s+(crate::)?constants::\*\s*;', text):
        raise SystemExit(f'forbidden wildcard constants alias in {path}')
print('ok')
PY
```

期待値:
- `ok`

追加で、root re-export・old root path・grouped root import・wildcard alias・`super::` / `self::` 経由の迂回参照が残っていないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
moved = 'REM|STATE_CIRCLE_SIZE|STATE_CIRCLE_GAP|STATE_CIRCLE_BOTTOM_MARGIN|STATE_CIRCLE_STROKE|MIN_QUBITS|MAX_QUBITS|MAX_STATE_COUNT|LINE_Y|LINE_GAP|CIRCUIT_PADDING|QUBIT_LABEL_WIDTH|QUBIT_LABEL_GAP|LINE_LEFT_OFFSET|LINE_RIGHT_OFFSET|GATE_SIZE|SLOT_SPACING|SNAP_DISTANCE|DRAG_REPAINT_BASE_SECS|DRAG_REPAINT_MIN_SECS|DRAG_REPAINT_MAX_SECS|DRAG_REPAINT_PUMP_FACTOR|PALETTE_SIZE|PALETTE_GAP|PALETTE_ROW_Y|PALETTE_GATES'
for path in Path('apps/web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+(constants::|self::constants::|super::constants::|crate::constants::|constants::\{|self::\{[^}]*\bconstants::|super::\{[^}]*\bconstants::|crate::\{[^}]*\bconstants::)', text, re.M | re.S):
        raise SystemExit(f'forbidden constants re-export in {path}')
    if re.search(r'use\s+(crate|self|super)::\{[^}]*\b(' + moved + r')\b', text, re.S):
        raise SystemExit(f'forbidden grouped old-path import in {path}')
    if re.search(r'use\s+((crate|self|super)::)?constants::\*\s*;', text):
        raise SystemExit(f'forbidden wildcard constants alias in {path}')
    if re.search(r'(crate|self|super)::(' + moved + r')\b', text):
        raise SystemExit(f'forbidden old constant path in {path}')
print('ok')
PY
```

期待値:
- `ok`

追加で、`lib.rs` に残すべきものが残っており、wasm export 属性とシグネチャが維持されていることを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/web/src/lib.rs').read_text()
for pattern in [
    r'^mod app;$',
    r'^mod colors;$',
    r'^mod constants;$',
    r'^mod gates;$',
    r'^mod gpu;$',
    r'^mod icons;$',
    r'^mod layout;$',
    r'^mod render;$',
    r'^mod shared;$',
    r'\#\[wasm_bindgen\]\s*pub async fn start\(canvas_id: &str\) -> Result<\(\), wasm_bindgen::JsValue>',
    r'\#\[wasm_bindgen\]\s*pub async fn read_state_vector\(\) -> Result<js_sys::Float32Array, wasm_bindgen::JsValue>',
]:
    if not re.search(pattern, lib, re.M):
        raise SystemExit(f'missing expected lib.rs symbol: {pattern}')
print('ok')
PY
```

期待値:
- `ok`

追加で、`constants.rs` の依存が allowlist 内に収まり、`GateKind` 以外への親参照や `super::` / `self::` 迂回がないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
text = Path('apps/web/src/constants.rs').read_text()
refs = sorted(set(re.findall(r'crate::[A-Za-z0-9_:]+', text)))
allowed = {'crate::gates::GateKind'}
if 'use crate::gates::GateKind;' not in text:
    raise SystemExit('missing explicit GateKind import in constants.rs')
if set(refs) - allowed:
    raise SystemExit(f'unexpected crate refs in constants.rs: {sorted(set(refs) - allowed)}')
if re.search(r'(super|self)::', text):
    raise SystemExit('unexpected super::/self:: path in constants.rs')
print('ok')
PY
```

期待値:
- `ok`

追加で、repo-wide changed-file allowlist・tests 未変更・LOC baseline を確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import subprocess
allowed = {
    'apps/web/src/lib.rs',
    'apps/web/src/constants.rs',
    'apps/web/src/app.rs',
    'apps/web/src/render.rs',
    'apps/web/src/layout.rs',
    'apps/web/src/gpu.rs',
}
changed = {
    line.strip().split(maxsplit=1)[1]
    for line in subprocess.check_output(
        ['git', 'status', '--short', '--untracked-files=all'],
        text=True,
    ).splitlines()
    if line.strip()
}
unexpected = changed - allowed
if unexpected:
    raise SystemExit(f'unexpected changed files: {sorted(unexpected)}')
loc = len(Path('apps/web/src/lib.rs').read_text().splitlines())
if loc >= 91:
    raise SystemExit(f'lib.rs LOC not reduced enough: {loc}')
print('ok')
PY
```
期待値:
- `ok`

## リスクと対策
- import 漏れ / visibility 漏れ
  - 対策: 早い段階で wasm `cargo check` を回し、task を細かく切り分ける。
- old root path の取り残し
  - 対策: Python による mechanical check を受け入れ条件に含める。
- `constants.rs` が convenience root alias の温床になる
  - 対策: root re-export 禁止を明示し、機械的に検査する。
- `PALETTE_GATES` の順序や定数値を誤って変える
  - 対策: pure extraction を明記し、symbol move check を受け入れ条件に含める。
- `lib.rs` に残すべき wasm export / module wiring を誤って動かす
  - 対策: remains check を受け入れ条件に含める。

## 完了時の期待状態
- `lib.rs` は module wiring + wasm exports を中心とした thin-entry に一段近づく。
- 共有定数と `PALETTE_GATES` は `constants.rs` にまとまり、利用側は `crate::constants::...` を直接参照する。
- 挙動変更なしのまま、将来の crate root 整理を続けやすい状態になる。
