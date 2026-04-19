# egui-web `lib.rs` thin-entry 化実装計画

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/egui-web/src/lib.rs` から gate/domain helper と `Colors` を internal module に純粋抽出し、挙動を変えずに `lib.rs` を thin-entry に近づける。

**Architecture:** この pass は code motion 中心の純粋抽出のみを行う。`apps/egui-web/src/gates.rs` に `GateKind` / `GateMatrix` / `GateParams` / `gate_*` helper を移し、`apps/egui-web/src/colors.rs` に `Colors` を移す。`lib.rs` には共有定数・共有 helper・wasm export を残し、既存 module は root re-export を使わず `crate::gates::...` / `crate::colors::...` を直接参照する。

**Tech Stack:** Rust, eframe/egui, egui_wgpu, wgpu/WebGPU, wasm-bindgen, Playwright, trunk

---

## この pass のファイル構成

- Create: `apps/egui-web/src/gates.rs`
  - `GateKind`
  - `GateMatrix`
  - `GateParams`
  - `gate_matrix`
  - `gate_params`
  - `gate_params_controlled`
- Create: `apps/egui-web/src/colors.rs`
  - `Colors`
- Modify: `apps/egui-web/src/lib.rs`
  - `mod gates;` / `mod colors;` を追加する
  - `PALETTE_GATES` など gate helper を使う箇所だけ `crate::gates::...` 参照に変える
  - 共有定数・`now_seconds`・`display_index_to_state_index`・`amplitude_qubits`・`color_rgba`・wasm export を残す
- Modify: `apps/egui-web/src/app.rs`
  - `crate::gates::{...}` / `crate::colors::Colors` を参照する
- Modify: `apps/egui-web/src/render.rs`
  - `crate::gates::GateKind` / `crate::colors::Colors` を参照する
- Modify: `apps/egui-web/src/gpu.rs`
  - `crate::gates::GateParams` / `crate::colors::Colors` を参照する
- Modify: `apps/egui-web/src/icons.rs`
  - `crate::gates::GateKind` / `crate::colors::Colors` を参照する
- Verify only: `apps/egui-web/tests/**`, `apps/egui-web/test-node/**`
  - この pass では変更しない
- Reference spec: `docs/superpowers/specs/2026-04-19-egui-web-lib-rs-thin-entry-design.md`

## ガードレール

- `cargo check` は **必ず** `--target wasm32-unknown-unknown` で実行する。
- この pass は **pure extraction only**。挙動変更、API 変更、最適化、命名変更は行わない。
- 共有定数は `lib.rs` に残す。
- `now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba` は `lib.rs` に残す。
- wasm export (`start`, `read_state_vector`) は `lib.rs` に残す。
- root re-export (`pub use`, `pub(crate) use`, `pub(super) use`) は追加しない。
- moved symbol 利用側は root alias ではなく `crate::gates::...` / `crate::colors::...` を直接参照する。
- 外部公開 (`pub`) は追加しない。crate 内可視性が必要な場合は **`pub(crate)` に統一** する。
- `gates.rs` の visibility は次に限定する。
  - `pub(crate)`: `GateKind`, `GateParams`, `gate_params`, `gate_params_controlled`, `GateKind::label`
  - private のまま維持: `GateMatrix`, `gate_matrix`（閉じられるなら）
- `colors.rs` の visibility は次に限定する。
  - `pub(crate)`: `Colors`, `Colors::new`, cross-module で直接読まれる field
- `apps/egui-web/tests` と `apps/egui-web/test-node` は変更しない。
- repo-wide aggregate check（例: `./scripts/check-all.sh`）はこの pass ではスコープ外とし、受け入れ条件は egui-web の wasm cargo check + Playwright + trunk build + diff check を正本とする。

### Task 1: `gates.rs` を追加し、gate/domain helper を先に抽出する

**Files:**
- Create: `apps/egui-web/src/gates.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Modify: `apps/egui-web/src/app.rs`
- Modify: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/gpu.rs`
- Modify: `apps/egui-web/src/icons.rs`
- Reference: `docs/superpowers/specs/2026-04-19-egui-web-lib-rs-thin-entry-design.md`

- [ ] **Step 1: gate/domain 抽出前のベースラインを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns|placed circuit gate keeps its visual while dragging another gate'
```
Expected:
- `cargo check --target wasm32-unknown-unknown` が success
- grep で一致した Playwright がすべて pass

- [ ] **Step 2: `gates.rs` を作成し、gate/domain helper を移す**

`apps/egui-web/src/gates.rs` を作成し、`apps/egui-web/src/lib.rs` に `mod gates;` を追加する。

`lib.rs` から次を移す。
- `GateKind`
- `GateMatrix`
- `GateParams`
- `gate_matrix`
- `gate_params`
- `gate_params_controlled`

方針:
- `GateKind`, `GateParams`, `gate_params`, `gate_params_controlled`, `GateKind::label` は **`pub(crate)`** にする
- `GateMatrix` と `gate_matrix` は `gates.rs` 内で閉じられるなら private のままにする
- 本体は import 解決と最小限の visibility 解決以外そのまま移す

初期 import 例:
```rust
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GateParams { /* moved as-is */ }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateKind { /* moved as-is */ }
```

- [ ] **Step 3: moved symbol 利用側の import を `crate::gates` 前提に更新する**

更新対象:
- `apps/egui-web/src/lib.rs`
- `apps/egui-web/src/app.rs`
- `apps/egui-web/src/render.rs`
- `apps/egui-web/src/gpu.rs`
- `apps/egui-web/src/icons.rs`

確認観点:
- `PALETTE_GATES` は `lib.rs` に残すが、型は `gates::GateKind` を使う
- `app.rs` は `GateKind`, `GateParams`, `gate_params`, `gate_params_controlled` を `crate::gates` から読む
- `render.rs` / `icons.rs` は `GateKind` を `crate::gates` から読む
- `gpu.rs` は `GateParams` を `crate::gates` から読む
- `crate::GateKind` / `crate::GateParams` の root 参照を残さない

- [ ] **Step 4: gate/domain 抽出後にコンパイルする**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- module / import / visibility 周りの error がなく success

- [ ] **Step 5: gate/domain 抽出後の focused 回帰を回す**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns|placed circuit gate keeps its visual while dragging another gate'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 6: gate/domain 抽出を commit する**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/gates.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/app.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/render.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/gpu.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/icons.rs
git commit -m "refactor: extract egui-web gate domain helpers"
```

### Task 2: `colors.rs` を追加し、`Colors` を抽出する

**Files:**
- Create: `apps/egui-web/src/colors.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Modify: `apps/egui-web/src/app.rs`
- Modify: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/gpu.rs`
- Modify: `apps/egui-web/src/icons.rs`
- Reference: `docs/superpowers/specs/2026-04-19-egui-web-lib-rs-thin-entry-design.md`

- [ ] **Step 1: `Colors` 抽出前のベースラインを再確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|palette panel keeps its corners and shadow while dragging|palette control gate keeps its icon while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 2: `colors.rs` を作成し、`Colors` を移す**

`apps/egui-web/src/colors.rs` を作成し、`apps/egui-web/src/lib.rs` に `mod colors;` を追加する。

`lib.rs` から次を移す。
- `Colors`
- `impl Colors`

方針:
- `Colors` と `Colors::new` は **`pub(crate)`** にする
- cross-module で直接読まれる field だけ **`pub(crate)`** にする
- 本体は import 解決と最小限の visibility 解決以外そのまま移す

- [ ] **Step 3: moved symbol 利用側の import を `crate::colors` 前提に更新する**

更新対象:
- `apps/egui-web/src/lib.rs`
- `apps/egui-web/src/app.rs`
- `apps/egui-web/src/render.rs`
- `apps/egui-web/src/gpu.rs`
- `apps/egui-web/src/icons.rs`

確認観点:
- `app.rs` の `Colors::new()` 呼び出しが `crate::colors::Colors` で解決すること
- `render.rs` / `gpu.rs` / `icons.rs` の `&Colors` と field access が `crate::colors::Colors` で成立すること
- `crate::Colors` の root 参照を残さないこと

- [ ] **Step 4: `Colors` 抽出後にコンパイルする**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- import / visibility / field access 周りの error がなく success

- [ ] **Step 5: `Colors` 抽出後の focused 回帰を回す**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|palette panel keeps its corners and shadow while dragging|palette control gate keeps its icon while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 6: `Colors` 抽出を commit する**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/colors.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/app.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/render.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/gpu.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/icons.rs
git commit -m "refactor: extract egui-web colors"
```

### Task 3: thin-entry pass 全体を検証し、次候補を決める

**Files:**
- Modify: none required unless review で修正が入る
- Reference: `apps/egui-web/src/lib.rs`
- Reference: `apps/egui-web/src/gates.rs`
- Reference: `apps/egui-web/src/colors.rs`
- Reference: `docs/superpowers/specs/2026-04-19-egui-web-lib-rs-thin-entry-design.md`

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

- [ ] **Step 3: gate/theme symbol move を機械的に確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'enum GateKind|struct GateMatrix|struct GateParams|fn gate_matrix|fn gate_params\(|fn gate_params_controlled|struct Colors' apps/egui-web/src/lib.rs apps/egui-web/src/gates.rs apps/egui-web/src/colors.rs
```
Expected:
- gate/domain helper は `gates.rs` 側にある
- `Colors` は `colors.rs` 側にある
- `lib.rs` にはそれらの定義が残っていない

- [ ] **Step 4: moved symbol 利用側が `crate::gates` / `crate::colors` を直接参照していることを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'crate::gates::|crate::colors::|use crate::gates::|use crate::colors::' apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/gpu.rs apps/egui-web/src/icons.rs apps/egui-web/src/lib.rs
```
Expected:
- `app.rs` / `render.rs` / `gpu.rs` / `icons.rs` / `lib.rs` は `gates.rs` / `colors.rs` を直接参照している
- root alias ではなく direct module path になっている

- [ ] **Step 5: root re-export がないことを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
for path in Path('apps/egui-web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+(gates::|colors::|self::gates::|self::colors::|crate::gates::|crate::colors::|gates::\{|colors::\{|self::\{[^}]*\b(gates::|colors::)|crate::\{[^}]*\b(gates::|colors::))', text, re.M | re.S):
        raise SystemExit(f'forbidden root re-export in {path}')
print('ok')
PY
```
Expected:
- `ok`

- [ ] **Step 6: forbidden root alias / root path reference が残っていないことを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
for path in Path('apps/egui-web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'use\s+crate::\{[^}]*\b(GateKind|GateParams|Colors)\b[^}]*\}', text, re.S):
        raise SystemExit(f'forbidden root grouped import in {path}')
    if re.search(r'crate::(GateKind|GateParams|Colors)\b', text):
        raise SystemExit(f'forbidden root path reference in {path}')
print('ok')
PY
```
Expected:
- `ok`
- grouped import を含め、forbidden root alias / root path reference は残っていない

- [ ] **Step 7: `lib.rs` に残すべき定数と wasm export が残っていることを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'const REM\b|const STATE_CIRCLE_SIZE\b|const MIN_QUBITS\b|const MAX_QUBITS\b|const MAX_STATE_COUNT\b|const LINE_Y\b|const GATE_SIZE\b|const SNAP_DISTANCE\b|const PALETTE_GAP\b|const PALETTE_ROW_Y\b|pub async fn start\b|pub async fn read_state_vector\b' apps/egui-web/src/lib.rs apps/egui-web/src/gates.rs apps/egui-web/src/colors.rs
```
Expected:
- 列挙した共有定数と `start` / `read_state_vector` は `lib.rs` のみにある
- `gates.rs` / `colors.rs` 側にはそれらの定義が現れない

- [ ] **Step 8: テストファイルが変更されていないことを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/egui-web/tests apps/egui-web/test-node
```
Expected:
- no output

- [ ] **Step 9: LOC を測り、次の候補を決める**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/egui-web/src/lib.rs apps/egui-web/src/gates.rs apps/egui-web/src/colors.rs apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/gpu.rs apps/egui-web/src/layout.rs apps/egui-web/src/icons.rs
```
Expected:
- `lib.rs` が pass 4 完了時より小さい
- 結果をレビュー依頼に添える

次候補の判断ルール:
- 共有 helper がまだ大きいなら → shared utility module への抽出
- crate root が十分薄くなったなら → 分割シリーズ完了

- [ ] **Step 10: reviewer に thin-entry pass のレビューを依頼する**

レビュー対象:
- `apps/egui-web/src/lib.rs`
- `apps/egui-web/src/gates.rs`
- `apps/egui-web/src/colors.rs`
- 関連 import 更新 (`app.rs`, `render.rs`, `gpu.rs`, `icons.rs`)
- fresh verification evidence
- symbol move / no-root-alias / LOC

レビュー観点:
- 純粋抽出を守れているか
- visibility が最小限に収まっているか
- `lib.rs` に shared helper / 定数 / wasm export が残っているか
- root re-export や root alias が増えていないか
- 分割シリーズを締めてもよい状態か、まだ shared helper pass が必要か

- [ ] **Step 11: review 指摘があれば最小修正して commit し、結果をまとめる**

もし reviewer 指摘があれば、必要最小限だけ修正して focused な commit を切る。
その後、次をまとめる:
- `gates.rs` に移したもの
- `colors.rs` に移したもの
- `lib.rs` に残したもの
- 新しい LOC
- 推奨される次パス候補
- fresh verification 結果
