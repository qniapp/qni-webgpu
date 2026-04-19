# egui-web `constants.rs` 抽出 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/egui-web/src/lib.rs` から共有定数と `PALETTE_GATES` を `apps/egui-web/src/constants.rs` に純粋抽出し、挙動を変えずに crate root をさらに thin-entry 化する。

**Architecture:** この pass は code motion のみを行う。`lib.rs` に残っている layout / drag / state / qubit 系定数と `PALETTE_GATES` を `constants.rs` へ移し、利用側は `crate::constants::...` を直接参照する。`lib.rs` には module wiring と wasm exports（`start`, `read_state_vector`）だけを残し、root re-export・grouped root import・wildcard alias は追加しない。

**Tech Stack:** Rust, eframe/egui, egui_wgpu, wgpu/WebGPU, wasm-bindgen, Playwright, trunk

---

## この pass のファイル構成

- Create: `apps/egui-web/src/constants.rs`
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
- Modify: `apps/egui-web/src/lib.rs`
  - `mod constants;` を追加する
  - 共有定数と `PALETTE_GATES` の定義を削除する
  - module wiring と wasm exports だけを残す
- Modify: `apps/egui-web/src/app.rs`
  - moved constants を `crate::constants::...` から参照する
- Modify: `apps/egui-web/src/render.rs`
  - moved constants を `crate::constants::...` から参照する
- Modify: `apps/egui-web/src/layout.rs`
  - moved constants を `crate::constants::...` から参照する
- Modify: `apps/egui-web/src/gpu.rs`
  - `MAX_STATE_COUNT` を `crate::constants::MAX_STATE_COUNT` から参照する
- Verify only: `apps/egui-web/src/colors.rs`, `apps/egui-web/src/shared.rs`, `apps/egui-web/src/gates.rs`, `apps/egui-web/src/icons.rs`
  - この pass では変更しない
- Verify only: `apps/egui-web/tests/**`, `apps/egui-web/test-node/**`
  - この pass では変更しない
- Reference spec: `docs/superpowers/specs/2026-04-20-egui-web-constants-rs-refactor-design.md`

## ガードレール

- 実装は **isolated worktree** で行う。
- 実行中は plan/spec/progress ファイルを編集しない。進捗管理はチェックボックスの閲覧または外部メモで行い、この pass の変更対象は `apps/egui-web/src/*` の allowlist に限定する。
- `cargo check` は **必ず** `--target wasm32-unknown-unknown` で実行する。
- `trunk build` は **必ず** `env PATH="$HOME/.cargo/bin:$PATH"` 付きで実行する。
- この pass は **pure extraction only**。挙動変更、API 変更、定数値変更、命名変更、並び替え、最適化は行わない。
- `constants.rs` に移すのは spec 記載の定数群と `PALETTE_GATES` のみ。
- `constants.rs` が参照してよい親シンボルは `crate::gates::GateKind` のみ。
- wasm export (`start`, `read_state_vector`) は `lib.rs` に残す。
- module wiring は `lib.rs` に残す。
- root re-export (`pub use`, `pub(crate) use`, `pub(super) use`) は追加しない。
- moved constants 利用側は root alias ではなく `crate::constants::...` を直接参照する。
- `use crate::{...moved constants...}` の grouped root import は残さない。
- `use constants::*;` / `use crate::constants::*;` / `use super::constants::*;` の wildcard alias は追加しない。
- 外部公開 (`pub`) は追加しない。moved constants と `PALETTE_GATES` は crate 内利用のため **`pub(crate) const`** に統一する。
- `apps/egui-web/src/colors.rs` / `shared.rs` / `gates.rs` / `icons.rs` は変更しない。
- `apps/egui-web/tests` と `apps/egui-web/test-node` は変更しない。
- repo-wide aggregate check（例: `./scripts/check-all.sh`）はこの pass ではスコープ外とし、受け入れ条件は egui-web の wasm cargo check + Playwright + trunk build + diff check を正本とする。

### Task 0: isolated worktree を準備する

**Files:**
- Modify: none
- Reference: `docs/superpowers/specs/2026-04-20-egui-web-constants-rs-refactor-design.md`
- Reference: `docs/superpowers/plans/2026-04-20-egui-web-constants-rs-refactor.md`

- [ ] **Step 1: clean な親 working tree を確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git status --short
```
Expected:
- no output

- [ ] **Step 2: 既存 worktree / branch が衝突しないことを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git worktree list
cd /home/yasuhito/Work/qni-webgpu && git branch --list refactor/egui-constants-pass
```
Expected:
- `git worktree list` に `egui-constants-pass` が出ない
- `git branch --list refactor/egui-constants-pass` が no output

- [ ] **Step 3: worktree と branch を作成する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git worktree add ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass -b refactor/egui-constants-pass master
```
Expected:
- `~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass` が作成される
- branch `refactor/egui-constants-pass` が作成される

- [ ] **Step 4: worktree 側で clean 状態を確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && git status --short
```
Expected:
- no output

### Task 1: `constants.rs` を追加し、共有定数と `PALETTE_GATES` を純粋抽出する

**Files:**
- Create: `apps/egui-web/src/constants.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Modify: `apps/egui-web/src/app.rs`
- Modify: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/layout.rs`
- Modify: `apps/egui-web/src/gpu.rs`
- Reference: `docs/superpowers/specs/2026-04-20-egui-web-constants-rs-refactor-design.md`

- [ ] **Step 1: 抽出前ベースラインを確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|H on q0 and q1 yields uniform superposition|palette panel keeps its corners and shadow while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- wasm `cargo check` が success
- 指定した focused Playwright がすべて pass

- [ ] **Step 2: `constants.rs` を作成し、定数群と `PALETTE_GATES` を移す**

`apps/egui-web/src/constants.rs` を作成し、`apps/egui-web/src/lib.rs` に `mod constants;` を追加する。

`lib.rs` から次を移す。
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

方針:
- moved constants と `PALETTE_GATES` は **`pub(crate) const`** にする
- `constants.rs` では `use crate::gates::GateKind;` だけを許可する
- 定数値・式・コメント・`PALETTE_GATES` 並び順はそのまま移す
- `lib.rs` からは定義のみを消し、wasm export 本体は触らない

初期 shape 例:
```rust
use crate::gates::GateKind;

pub(crate) const REM: f32 = 32.0;
pub(crate) const MAX_STATE_COUNT: usize = 1 << MAX_QUBITS;

pub(crate) const PALETTE_GATES: [GateKind; 15] = [
    GateKind::H,
    GateKind::Control,
    // ... moved as-is
];
```

- [ ] **Step 3: moved constants 利用側を `crate::constants::...` 前提に更新する**

更新対象:
- `apps/egui-web/src/app.rs`
- `apps/egui-web/src/render.rs`
- `apps/egui-web/src/layout.rs`
- `apps/egui-web/src/gpu.rs`
- `apps/egui-web/src/lib.rs`

確認観点:
- `app.rs` は drag / palette / qubit / size 系定数を `crate::constants` から解決する
- `render.rs` は layout / palette / state circle 系定数を `crate::constants` から解決する
- `layout.rs` は layout 定数を `crate::constants` から解決する
- `gpu.rs` は `MAX_STATE_COUNT` を `crate::constants::MAX_STATE_COUNT` から解決する
- `crate::REM` / `crate::GATE_SIZE` / `crate::MAX_STATE_COUNT` / `crate::PALETTE_GATES` は残さない
- `use crate::{...moved constants...}` の grouped root import は残さない
- wildcard constants alias は追加しない

- [ ] **Step 4: 抽出直後にコンパイルして import / visibility 崩れを切り分ける**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- module / import / visibility 周りの error がなく success

- [ ] **Step 5: focused 回帰を回す**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|H on q0 and q1 yields uniform superposition|palette panel keeps its corners and shadow while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- 一致したテストがすべて pass

### Task 2: pass 全体を検証し、mechanical check を完了する

**Files:**
- Modify: none required unless review で修正が入る
- Reference: `apps/egui-web/src/constants.rs`
- Reference: `apps/egui-web/src/lib.rs`
- Reference: `apps/egui-web/src/app.rs`
- Reference: `apps/egui-web/src/render.rs`
- Reference: `apps/egui-web/src/layout.rs`
- Reference: `apps/egui-web/src/gpu.rs`
- Reference: `docs/superpowers/specs/2026-04-20-egui-web-constants-rs-refactor-design.md`

- [ ] **Step 1: pass 全体の完全検証を実行する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && pnpm exec playwright test
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" trunk build
```
Expected:
- 3 コマンドすべて success

- [ ] **Step 2: patch hygiene を確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && git diff --check
```
Expected:
- no output

- [ ] **Step 3: constant move と pure extraction（値・式不変、`constants.rs` 内順序維持）を機械的に確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/egui-web/src/lib.rs').read_text()
constants = Path('apps/egui-web/src/constants.rs').read_text()
order = [
    'REM',
    'STATE_CIRCLE_SIZE',
    'STATE_CIRCLE_GAP',
    'STATE_CIRCLE_BOTTOM_MARGIN',
    'STATE_CIRCLE_STROKE',
    'MIN_QUBITS',
    'MAX_QUBITS',
    'MAX_STATE_COUNT',
    'LINE_Y',
    'LINE_GAP',
    'CIRCUIT_PADDING',
    'QUBIT_LABEL_WIDTH',
    'QUBIT_LABEL_GAP',
    'LINE_LEFT_OFFSET',
    'LINE_RIGHT_OFFSET',
    'GATE_SIZE',
    'SLOT_SPACING',
    'SNAP_DISTANCE',
    'DRAG_REPAINT_BASE_SECS',
    'DRAG_REPAINT_MIN_SECS',
    'DRAG_REPAINT_MAX_SECS',
    'DRAG_REPAINT_PUMP_FACTOR',
    'PALETTE_SIZE',
    'PALETTE_GAP',
    'PALETTE_ROW_Y',
]
expected_rhs = {
    'REM': '32.0',
    'STATE_CIRCLE_SIZE': '1.25 * REM',
    'STATE_CIRCLE_GAP': '0.5 * REM',
    'STATE_CIRCLE_BOTTOM_MARGIN': '2.0 * REM',
    'STATE_CIRCLE_STROKE': '2.0',
    'MIN_QUBITS': '2',
    'MAX_QUBITS': '16',
    'MAX_STATE_COUNT': '1 << MAX_QUBITS',
    'LINE_Y': '6.5 * REM',
    'LINE_GAP': '1.5 * REM',
    'CIRCUIT_PADDING': '2.0 * REM',
    'QUBIT_LABEL_WIDTH': '3.0 * 14.0',
    'QUBIT_LABEL_GAP': '0.5 * REM',
    'LINE_LEFT_OFFSET': 'CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP',
    'LINE_RIGHT_OFFSET': 'CIRCUIT_PADDING',
    'GATE_SIZE': '1.0 * REM',
    'SLOT_SPACING': 'GATE_SIZE * 1.5',
    'SNAP_DISTANCE': '0.5625 * REM',
    'DRAG_REPAINT_BASE_SECS': '0.01',
    'DRAG_REPAINT_MIN_SECS': '0.004',
    'DRAG_REPAINT_MAX_SECS': '1.0 / 30.0',
    'DRAG_REPAINT_PUMP_FACTOR': '0.1',
    'PALETTE_SIZE': 'GATE_SIZE',
    'PALETTE_GAP': '0.5 * REM',
    'PALETTE_ROW_Y': '2.0 * REM',
}
def normalize(expr: str) -> str:
    return re.sub(r'\s+', ' ', expr.strip())
positions = []
for name in order:
    if re.search(r'(?:^|\n)(?:pub\(crate\)\s+)?const\s+' + re.escape(name) + r'\b', lib):
        raise SystemExit(f'constant still present in lib.rs: {name}')
    match = re.search(
        r'pub\(crate\)\s+const\s+' + re.escape(name) + r'\s*:\s*[^=]+?=\s*(.*?);',
        constants,
        re.S,
    )
    if not match:
        raise SystemExit(f'constant missing in constants.rs: {name}')
    if normalize(match.group(1)) != expected_rhs[name]:
        raise SystemExit(f'constant expression changed for {name}: {normalize(match.group(1))}')
    positions.append(match.start())
if positions != sorted(positions):
    raise SystemExit('constant order changed in constants.rs')
expected_palette = [
    'GateKind::H',
    'GateKind::Control',
    'GateKind::X',
    'GateKind::Y',
    'GateKind::Z',
    'GateKind::SqrtX',
    'GateKind::S',
    'GateKind::SDagger',
    'GateKind::T',
    'GateKind::TDagger',
    'GateKind::Phase',
    'GateKind::Rx',
    'GateKind::Ry',
    'GateKind::Rz',
    'GateKind::Swap',
]
if re.search(r'(?:^|\n)(?:pub\(crate\)\s+)?const\s+PALETTE_GATES\b', lib):
    raise SystemExit('PALETTE_GATES still present in lib.rs')
match = re.search(r'pub\(crate\)\s+const\s+PALETTE_GATES\s*:\s*\[GateKind;\s*15\]\s*=\s*\[(.*?)\];', constants, re.S)
if not match:
    raise SystemExit('PALETTE_GATES missing in constants.rs')
entries = [normalize(x).rstrip(',') for x in match.group(1).splitlines() if normalize(x)]
if entries != expected_palette:
    raise SystemExit(f'PALETTE_GATES changed: {entries}')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 4: 利用側が `crate::constants::...` を直接使い、grouped root import / wildcard alias が残っていないことを確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && python - <<'PY'
from pathlib import Path
import re
checks = {
    'apps/egui-web/src/app.rs': ['PALETTE_GATES', 'GATE_SIZE', 'MAX_QUBITS'],
    'apps/egui-web/src/render.rs': ['PALETTE_GATES', 'STATE_CIRCLE_SIZE', 'CIRCUIT_PADDING'],
    'apps/egui-web/src/layout.rs': ['LINE_LEFT_OFFSET', 'SLOT_SPACING', 'GATE_SIZE'],
    'apps/egui-web/src/gpu.rs': ['MAX_STATE_COUNT'],
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
Expected:
- `ok`

- [ ] **Step 5: root re-export・old path・`super::` / `self::` 迂回参照がないことを確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && python - <<'PY'
from pathlib import Path
import re
moved = 'REM|STATE_CIRCLE_SIZE|STATE_CIRCLE_GAP|STATE_CIRCLE_BOTTOM_MARGIN|STATE_CIRCLE_STROKE|MIN_QUBITS|MAX_QUBITS|MAX_STATE_COUNT|LINE_Y|LINE_GAP|CIRCUIT_PADDING|QUBIT_LABEL_WIDTH|QUBIT_LABEL_GAP|LINE_LEFT_OFFSET|LINE_RIGHT_OFFSET|GATE_SIZE|SLOT_SPACING|SNAP_DISTANCE|DRAG_REPAINT_BASE_SECS|DRAG_REPAINT_MIN_SECS|DRAG_REPAINT_MAX_SECS|DRAG_REPAINT_PUMP_FACTOR|PALETTE_SIZE|PALETTE_GAP|PALETTE_ROW_Y|PALETTE_GATES'
lib = Path('apps/egui-web/src/lib.rs').read_text()
if re.search(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\b', lib, re.M):
    raise SystemExit('forbidden root re-export in lib.rs')
for path in Path('apps/egui-web/src').glob('*.rs'):
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
Expected:
- `ok`

- [ ] **Step 6: `lib.rs` remains check と wasm export 属性/シグネチャ維持を確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/egui-web/src/lib.rs').read_text()
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
Expected:
- `ok`

- [ ] **Step 7: `constants.rs` の依存と可視性を確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && python - <<'PY'
from pathlib import Path
import re
text = Path('apps/egui-web/src/constants.rs').read_text()
refs = sorted(set(re.findall(r'crate::[A-Za-z0-9_:]+', text)))
allowed = {'crate::gates::GateKind'}
required = [
    r'pub\(crate\) const REM\b',
    r'pub\(crate\) const MAX_STATE_COUNT\b',
    r'pub\(crate\) const PALETTE_GATES\b',
]
if 'use crate::gates::GateKind;' not in text:
    raise SystemExit('missing explicit GateKind import in constants.rs')
for pattern in required:
    if not re.search(pattern, text):
        raise SystemExit(f'missing expected pub(crate) const: {pattern}')
if set(refs) - allowed:
    raise SystemExit(f'unexpected crate refs in constants.rs: {sorted(set(refs) - allowed)}')
if re.search(r'(super|self)::', text):
    raise SystemExit('unexpected super::/self:: path in constants.rs')
if re.search(r'\bpub const\b', text):
    raise SystemExit('constants.rs unexpectedly exposes pub const')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 8: repo-wide changed-file allowlist・tests 未変更・LOC baseline を確認する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && python - <<'PY'
from pathlib import Path
import subprocess
allowed = {
    'apps/egui-web/src/lib.rs',
    'apps/egui-web/src/constants.rs',
    'apps/egui-web/src/app.rs',
    'apps/egui-web/src/render.rs',
    'apps/egui-web/src/layout.rs',
    'apps/egui-web/src/gpu.rs',
}
changed = set()
for cmd in [
    ['git', 'diff', '--name-only', '--diff-filter=ACMRTD'],
    ['git', 'diff', '--cached', '--name-only', '--diff-filter=ACMRTD'],
    ['git', 'ls-files', '--others', '--exclude-standard'],
]:
    changed.update(
        line.strip()
        for line in subprocess.check_output(cmd, text=True).splitlines()
        if line.strip()
    )
unexpected = changed - allowed
if unexpected:
    raise SystemExit(f'unexpected changed files: {sorted(unexpected)}')
loc = len(Path('apps/egui-web/src/lib.rs').read_text().splitlines())
if loc >= 91:
    raise SystemExit(f'lib.rs LOC not reduced enough: {loc}')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 9: LOC を記録し、次候補を明確化する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && wc -l apps/egui-web/src/lib.rs apps/egui-web/src/constants.rs apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/layout.rs apps/egui-web/src/gpu.rs
```
Expected:
- `lib.rs` が baseline 91 LOC より減っている
- `constants.rs` 追加後も責務線が明確である

記録内容:
- `lib.rs` / `constants.rs` の LOC
- 次候補（例: lib.rs lane 完了扱い、または plain Chrome 差分調査への復帰）

- [ ] **Step 10: review 用の前提を満たした状態で commit する**

Run:
```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && git add apps/egui-web/src/constants.rs apps/egui-web/src/lib.rs apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/layout.rs apps/egui-web/src/gpu.rs
cd ~/.config/superpowers/worktrees/qni-webgpu/egui-constants-pass && git commit -m "refactor: extract egui-web constants"
```
Expected:
- review 前の clean commit が作成される
- 以後の spec compliance / code quality review はこの commit を対象に行える
