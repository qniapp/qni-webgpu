# egui-web `shared.rs` 抽出 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/egui-web/src/lib.rs` から shared helper を `apps/egui-web/src/shared.rs` に純粋抽出し、挙動を変えずに crate root をさらに thin-entry 化する。

**Architecture:** この pass は code motion のみを行う。`now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba` を `shared.rs` へ移し、利用側は `crate::shared::...` を直接参照する。`lib.rs` には共有定数、`PALETTE_GATES`、wasm exports、module wiring だけを残し、root re-export は追加しない。

**Tech Stack:** Rust, eframe/egui, egui_wgpu, wgpu/WebGPU, wasm-bindgen, Playwright, trunk

---

## この pass のファイル構成

- Create: `apps/egui-web/src/shared.rs`
  - `now_seconds`
  - `display_index_to_state_index`
  - `amplitude_qubits`
  - `color_rgba`
- Modify: `apps/egui-web/src/lib.rs`
  - `mod shared;` を追加する
  - shared helper 定義を削除する
  - 共有定数、`PALETTE_GATES`、wasm exports、module wiring を残す
- Modify: `apps/egui-web/src/app.rs`
  - `now_seconds` を `crate::shared::now_seconds` から参照する
- Modify: `apps/egui-web/src/render.rs`
  - `amplitude_qubits` と `display_index_to_state_index` を `crate::shared` から参照する
- Modify: `apps/egui-web/src/colors.rs`
  - `color_rgba` を `crate::shared` から参照する
- Verify only: `apps/egui-web/tests/**`, `apps/egui-web/test-node/**`
  - この pass では変更しない
- Reference spec: `docs/superpowers/specs/2026-04-19-egui-web-shared-rs-refactor-design.md`

## ガードレール

- `cargo check` は **必ず** `--target wasm32-unknown-unknown` で実行する。
- この pass は **pure extraction only**。挙動変更、API 変更、命名変更、最適化は行わない。
- `apps/egui-web/src/shared.rs` に移すのは次の 4 helper のみ。
  - `now_seconds`
  - `display_index_to_state_index`
  - `amplitude_qubits`
  - `color_rgba`
- 共有定数は `lib.rs` に残す。
- `PALETTE_GATES` は `lib.rs` に残す。
- wasm export (`start`, `read_state_vector`) は `lib.rs` に残す。
- root re-export (`pub use`, `pub(crate) use`, `pub(super) use`) は追加しない。
- moved helper 利用側は root alias ではなく `crate::shared::...` を直接参照する。
- 外部公開 (`pub`) は追加しない。shared helper は crate 内利用のため **`pub(crate)`** に統一する。
- `apps/egui-web/tests` と `apps/egui-web/test-node` は変更しない。
- repo-wide aggregate check（例: `./scripts/check-all.sh`）はこの pass ではスコープ外とし、受け入れ条件は egui-web の wasm cargo check + Playwright + trunk build + diff check を正本とする。

### Task 1: `shared.rs` を追加し、shared helper を純粋抽出する

**Files:**
- Create: `apps/egui-web/src/shared.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Modify: `apps/egui-web/src/app.rs`
- Modify: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/colors.rs`
- Reference: `docs/superpowers/specs/2026-04-19-egui-web-shared-rs-refactor-design.md`

- [ ] **Step 1: 抽出前のベースラインを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|H on q0 and q1 yields uniform superposition|palette panel keeps its corners and shadow while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- wasm `cargo check` が success
- 指定した focused Playwright がすべて pass

- [ ] **Step 2: `shared.rs` を作成し、4 helper を移す**

`apps/egui-web/src/shared.rs` を作成し、`apps/egui-web/src/lib.rs` に `mod shared;` を追加する。

`lib.rs` から次を移す。
- `now_seconds`
- `display_index_to_state_index`
- `amplitude_qubits`
- `color_rgba`

方針:
- helper 本体は import 解決と visibility 調整以外そのまま移す
- helper は **`pub(crate)`** にする
- `shared.rs` に新規 helper や定数は足さない

初期 shape 例:
```rust
#[cfg(target_arch = "wasm32")]
pub(crate) fn now_seconds() -> f64 { /* moved as-is */ }

pub(crate) fn amplitude_qubits(len: usize) -> usize { /* moved as-is */ }
```

- [ ] **Step 3: 利用側を `crate::shared::...` 前提に更新する**

更新対象:
- `apps/egui-web/src/app.rs`
- `apps/egui-web/src/render.rs`
- `apps/egui-web/src/colors.rs`
- `apps/egui-web/src/lib.rs`

確認観点:
- `app.rs` は `now_seconds` を `crate::shared` から解決する
- `render.rs` は `amplitude_qubits` / `display_index_to_state_index` を `crate::shared` から解決する
- `colors.rs` は `color_rgba` を `crate::shared` から解決する
- `crate::now_seconds` / `crate::amplitude_qubits` / `crate::display_index_to_state_index` / `crate::color_rgba` の root 参照を残さない
- root re-export は追加しない

- [ ] **Step 4: 抽出直後にコンパイルして import / visibility 崩れを切り分ける**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- module / import / visibility 周りの error がなく success

- [ ] **Step 5: focused 回帰を回す**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|H on q0 and q1 yields uniform superposition|palette panel keeps its corners and shadow while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 6: shared helper 抽出を commit する**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/shared.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/app.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/render.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/colors.rs
git commit -m "refactor: extract egui-web shared helpers"
```

### Task 2: pass 全体を検証し、mechanical check を完了する

**Files:**
- Modify: none required unless review で修正が入る
- Reference: `apps/egui-web/src/lib.rs`
- Reference: `apps/egui-web/src/shared.rs`
- Reference: `apps/egui-web/src/app.rs`
- Reference: `apps/egui-web/src/render.rs`
- Reference: `apps/egui-web/src/colors.rs`
- Reference: `docs/superpowers/specs/2026-04-19-egui-web-shared-rs-refactor-design.md`

- [ ] **Step 1: pass 全体の完全検証を実行する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && trunk build
```
Expected:
- 3 コマンドすべて success

- [ ] **Step 2: patch hygiene を確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```
Expected:
- no output

- [ ] **Step 3: helper move を機械的に確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/egui-web/src/lib.rs').read_text()
shared = Path('apps/egui-web/src/shared.rs').read_text()
for pattern in [
    r'fn now_seconds',
    r'fn display_index_to_state_index',
    r'fn amplitude_qubits',
    r'fn color_rgba',
]:
    if re.search(pattern, lib):
        raise SystemExit(f'helper still present in lib.rs: {pattern}')
    if not re.search(pattern, shared):
        raise SystemExit(f'helper missing from shared.rs: {pattern}')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 4: agreed direct-user mapping を spec どおり機械的に確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
checks = {
    'apps/egui-web/src/app.rs': [
        'use crate::shared::now_seconds;',
    ],
    'apps/egui-web/src/render.rs': [
        'use crate::shared::{amplitude_qubits, display_index_to_state_index};',
    ],
    'apps/egui-web/src/colors.rs': [
        'crate::shared::color_rgba(',
    ],
}
for path, needles in checks.items():
    text = Path(path).read_text()
    for needle in needles:
        if needle not in text:
            raise SystemExit(f'missing expected shared mapping in {path}: {needle}')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 5: `shared.rs` の helper visibility を確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
text = Path('apps/egui-web/src/shared.rs').read_text()
required = [
    r'pub\(crate\) fn now_seconds',
    r'pub\(crate\) fn display_index_to_state_index',
    r'pub\(crate\) fn amplitude_qubits',
    r'pub\(crate\) fn color_rgba',
]
for pattern in required:
    if not re.search(pattern, text):
        raise SystemExit(f'missing expected pub(crate) helper: {pattern}')
if re.search(r'\bpub fn\b', text):
    raise SystemExit('shared.rs unexpectedly exposes pub fn')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 6: no-root-reexport と old root path 不在を確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
for path in Path('apps/egui-web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+(shared::|self::shared::|crate::shared::|shared::\{|self::\{[^}]*\bshared::|crate::\{[^}]*\bshared::)', text, re.M | re.S):
        raise SystemExit(f'forbidden root re-export in {path}')
    if re.search(r'crate::(now_seconds|display_index_to_state_index|amplitude_qubits|color_rgba)\b', text):
        raise SystemExit(f'forbidden old root path in {path}')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 7: `lib.rs` remains check を行う**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/egui-web/src/lib.rs').read_text()
shared = Path('apps/egui-web/src/shared.rs').read_text()
lib_patterns = [
    r'const REM\b',
    r'const STATE_CIRCLE_SIZE\b',
    r'const STATE_CIRCLE_GAP\b',
    r'const STATE_CIRCLE_BOTTOM_MARGIN\b',
    r'const STATE_CIRCLE_STROKE\b',
    r'const MIN_QUBITS\b',
    r'const MAX_QUBITS\b',
    r'const MAX_STATE_COUNT\b',
    r'const LINE_Y\b',
    r'const LINE_GAP\b',
    r'const CIRCUIT_PADDING\b',
    r'const QUBIT_LABEL_WIDTH\b',
    r'const QUBIT_LABEL_GAP\b',
    r'const LINE_LEFT_OFFSET\b',
    r'const LINE_RIGHT_OFFSET\b',
    r'const GATE_SIZE\b',
    r'const SLOT_SPACING\b',
    r'const SNAP_DISTANCE\b',
    r'const DRAG_REPAINT_BASE_SECS\b',
    r'const DRAG_REPAINT_MIN_SECS\b',
    r'const DRAG_REPAINT_MAX_SECS\b',
    r'const DRAG_REPAINT_PUMP_FACTOR\b',
    r'const PALETTE_SIZE\b',
    r'const PALETTE_GAP\b',
    r'const PALETTE_ROW_Y\b',
    r'const PALETTE_GATES\b',
    r'pub async fn start\b',
    r'pub async fn read_state_vector\b',
]
for pattern in lib_patterns:
    if not re.search(pattern, lib):
        raise SystemExit(f'missing expected lib.rs symbol: {pattern}')
    if re.search(pattern, shared):
        raise SystemExit(f'symbol moved into shared.rs unexpectedly: {pattern}')
if re.search(r'^const\s+', shared, re.M):
    raise SystemExit('shared.rs unexpectedly contains const definitions')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 8: tests 未変更を確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git status --short --untracked-files=all -- apps/egui-web/tests apps/egui-web/test-node
```
Expected:
- no output

- [ ] **Step 9: LOC を記録し、次候補を明確化する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/egui-web/src/lib.rs apps/egui-web/src/shared.rs apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/colors.rs
```
Expected:
- `lib.rs` が current head（140 LOC）より減っている
- `shared.rs` の追加後も責務線が明確である

記録内容:
- `lib.rs` / `shared.rs` の LOC
- 次候補が残るなら、その候補（例: constants を含まないさらに小さな root cleanup ではなく、現時点では thin-entry pass 完了扱いかどうか）を明記する
