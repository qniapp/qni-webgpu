# egui-web `render.rs` 分割（第3パス）実装計画

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/egui-web/src/lib.rs` から circuit / palette / state panel の描画責務を `apps/egui-web/src/render.rs` に純粋抽出し、挙動を変えずに `lib.rs` をさらに縮小する。

**Architecture:** この pass は code motion 中心の純粋抽出のみを行う。`QniApp`、input / drag、update loop、gate/domain helper、wasm export は `lib.rs` に残し、描画 helper と state panel 用の layout/cache 型だけを `render.rs` へ寄せる。

**Tech Stack:** Rust, eframe/egui, egui_wgpu, wgpu/WebGPU, wasm-bindgen, Playwright, trunk

---

## この pass のファイル構成

- Create: `apps/egui-web/src/render.rs`
  - `circuit_content_height`
  - `draw_circuit`
  - `draw_palette`
  - `state_panel_layout`
  - `clamp_state_panel_offset`
  - `state_instances_for`
  - `draw_state_vector`
  - `StatePanelLayout`
  - `StateInstanceKey`
  - `StateInstanceCache`
- Modify: `apps/egui-web/src/lib.rs`
  - `mod render;` を追加する
  - `render.rs` へ移した型を import する
  - `QniApp` field が `render::StateInstanceCache` を参照するようにする
  - `update` は `lib.rs` に残したまま、render helper を呼ぶ形に保つ
- Verify: `apps/egui-web/tests/egui-web.spec.js`
  - この pass では変更しない
- Reference spec: `docs/superpowers/specs/2026-04-15-egui-web-render-rs-refactor-design.md`

## ガードレール

- `cargo check` は **必ず** `--target wasm32-unknown-unknown` で実行する。
- 描画内容、shadow、hover、fast-drag、state panel handle、clip rect、callback 作成順は変更しない。
- `update`、`handle_input`、`schedule_drag_repaint` は `lib.rs` に残す。
- `Colors` はこの pass では `lib.rs` に残す。
- `apps/egui-web/tests/egui-web.spec.js` は変更しない。
- 可視性変更は必要最小限に限定し、外部公開 (`pub`) は追加しない。必要なら `pub(super)` または `pub(crate)` までに留める。
- `lib.rs` 側の `update` から呼ばれる extracted method は `pub(super)` を基本とする。
- `StatePanelLayout` と `StateInstanceCache` は、`QniApp` field / call site に必要な最小限だけ `pub(super)` を使う。
- `StatePanelLayout` の field は、`lib.rs` 側で直接読むもの（少なくとも `state_rect` と `handle_height`）だけを見せ、それ以外は可能なら `render.rs` に閉じる。
- repo-wide aggregate check（例: `./scripts/check-all.sh`）はこの pass ではスコープ外とし、受け入れ条件は egui-web の wasm cargo check + Playwright + trunk build + diff check を正本とする。

### Task 1: `render.rs` を追加し、state panel 用の型を先に移す

**Files:**
- Create: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Reference: `docs/superpowers/specs/2026-04-15-egui-web-render-rs-refactor-design.md`

- [ ] **Step 1: 抽出前のベースラインを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|palette panel keeps its corners and shadow while dragging|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- `cargo check --target wasm32-unknown-unknown` が success
- grep で一致した Playwright がすべて pass

- [ ] **Step 2: `render.rs` を作成し、`mod render;` を追加する**

`apps/egui-web/src/render.rs` を作成し、`apps/egui-web/src/lib.rs` の先頭付近に `mod render;` を追加する。

初期スケルトン例:
```rust
use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use crate::gpu::{RenderColors, StateInstance, StateVectorCallback};
use crate::icons::{draw_gate_body, draw_gate_body_fast};
use crate::layout::{layout_metrics, nearest_slot_index, LayoutMetrics};
use crate::{
    amplitude_qubits, display_index_to_state_index, should_use_fast_gate_body, Colors, GateKind,
    PlacedGate, QniApp, CIRCUIT_PADDING, GATE_SIZE, LINE_GAP, LINE_Y, PALETTE_GAP,
    PALETTE_ROW_Y, PALETTE_SIZE, REM, SNAP_DISTANCE, STATE_CIRCLE_BOTTOM_MARGIN,
    STATE_CIRCLE_GAP, STATE_CIRCLE_SIZE, STATE_CIRCLE_STROKE,
};
```

- [ ] **Step 3: state panel 用の型を `render.rs` に移す**

`lib.rs` から以下を `render.rs` に移す。
- `StatePanelLayout`
- `StateInstanceKey`
- `StateInstanceCache`

方針:
- `StateInstanceKey` は可能なら private のまま
- `StateInstanceCache` は `QniApp` field で使える最小限の visibility
- `StatePanelLayout` は `update` が直接読む field だけ最小限で見せる

- [ ] **Step 4: `QniApp` field と import を最小限調整してコンパイルする**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- module / type visibility / field type 周りの error がなく success

- [ ] **Step 5: 型抽出後の focused 回帰を回す**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'egui webgpu canvas renders content|palette panel keeps its corners and shadow while dragging'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 6: 型抽出を commit する**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/render.rs
git commit -m "refactor: extract egui-web render types"
```

### Task 2: circuit / palette 描画 helper を抽出する

**Files:**
- Modify: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Verify: `apps/egui-web/tests/egui-web.spec.js`

- [ ] **Step 1: drag / palette 系の baseline を再確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'palette panel keeps its corners and shadow while dragging|palette control gate keeps its icon while dragging|dragged palette gate keeps rounded corners|dragged x gate keeps the same visual as after drop'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 2: `circuit_content_height`、`draw_circuit`、`draw_palette` を `render.rs` に移す**

`lib.rs` から以下を `render.rs` に移す。
- `circuit_content_height`
- `draw_circuit`
- `draw_palette`

方針:
- `impl QniApp` の method として移す
- `lib.rs` 側の `update` から呼べるよう、必要最小限で `pub(super)` を使う
- `HashMap` / `Ordering` / `PlacedGate` / `GateKind` 依存はそのまま持ち込む
- 描画本体は import / visibility 解決以外を変更しない

- [ ] **Step 3: circuit / palette 抽出後にコンパイルする**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- unresolved symbol / private method error がなく success

- [ ] **Step 4: drag / palette 回帰を再実行する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'palette panel keeps its corners and shadow while dragging|palette control gate keeps its icon while dragging|dragged palette gate keeps rounded corners|dragged x gate keeps the same visual as after drop'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 5: circuit / palette 抽出を commit する**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/render.rs
git commit -m "refactor: extract egui-web render methods"
```

### Task 3: state panel helper を抽出し、`update` との接続を保つ

**Files:**
- Modify: `apps/egui-web/src/render.rs`
- Modify: `apps/egui-web/src/lib.rs`
- Verify: `apps/egui-web/tests/egui-web.spec.js`

- [ ] **Step 1: state vector / readback 系の baseline を再確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 2: state panel helper を `render.rs` に移す**

`lib.rs` から以下を `render.rs` に移す。
- `state_panel_layout`
- `clamp_state_panel_offset`
- `state_instances_for`
- `draw_state_vector`

方針:
- `draw_state_vector` の callback 生成順と `gpu::RenderColors` / `gpu::StateVectorCallback` 使用は変更しない
- `state_instances_for` の cache hit 条件と `StateInstance` 配列生成順は変更しない
- `layout_metrics(...)` + `collect_gate_params(...)` の利用位置は変えない
- `update` の handle drag / recompute 判定ロジックは `lib.rs` に残す

- [ ] **Step 3: `update` 側 call site を最小限調整する**

確認観点:
- `let state_layout = self.state_panel_layout(...)` がそのまま使えること
- `self.clamp_state_panel_offset(...)` が呼べること
- `state_layout.state_rect` と `state_layout.handle_height` へのアクセスだけが必要最小限で維持されること
- `draw_state_vector(...)` 呼び出しが変わらないこと

- [ ] **Step 4: state panel 抽出後にコンパイルする**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```
Expected:
- type visibility / field visibility / callback import 周りを含めて success

- [ ] **Step 5: state vector / readback 回帰を再実行する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns|default chromium shows a visible WebGPU error instead of a blank page'
```
Expected:
- 一致したテストがすべて pass

- [ ] **Step 6: state panel 抽出を commit する**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/egui-web/src/render.rs
git commit -m "refactor: extract egui-web state panel rendering"
```

### Task 4: pass 3 全体を検証し、pass 4 候補を決める

**Files:**
- Modify: none required unless review で修正が入る
- Reference: `apps/egui-web/src/lib.rs`
- Reference: `apps/egui-web/src/render.rs`
- Reference: `docs/superpowers/specs/2026-04-15-egui-web-render-rs-refactor-design.md`

- [ ] **Step 1: pass 3 の完全検証を実行する**

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

- [ ] **Step 3: render シンボルが意図どおり移動したことを機械的に確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'fn circuit_content_height\b|fn draw_circuit\b|fn draw_palette\b|fn state_panel_layout\b|fn clamp_state_panel_offset\b|fn state_instances_for\b|fn draw_state_vector\b|struct StatePanelLayout\b|struct StateInstanceKey\b|struct StateInstanceCache\b' apps/egui-web/src/lib.rs apps/egui-web/src/render.rs
```
Expected:
- 列挙した render シンボルは `render.rs` 側にある
- `lib.rs` にはそれらの定義が残っていない

- [ ] **Step 4: `lib.rs` に残すべき app/export シンボルが残っていることを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'fn handle_input\b|fn schedule_drag_repaint\b|impl eframe::App for QniApp|struct Colors\b|pub async fn start|pub async fn read_state_vector|fn gate_params\b|fn gate_params_controlled\b|fn gate_matrix\b' apps/egui-web/src/lib.rs apps/egui-web/src/render.rs
```
Expected:
- `handle_input`、`schedule_drag_repaint`、`impl eframe::App for QniApp`、`Colors`、`start`、`read_state_vector`、`gate_params`、`gate_params_controlled`、`gate_matrix` は `lib.rs` のみにある
- `render.rs` 側にこれらの定義は現れない

- [ ] **Step 5: テストファイルが変更されていないことを確認する**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/egui-web/tests/egui-web.spec.js
```
Expected:
- no output

- [ ] **Step 6: LOC を測り、次の候補を決める**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/egui-web/src/lib.rs apps/egui-web/src/render.rs apps/egui-web/src/gpu.rs apps/egui-web/src/layout.rs apps/egui-web/src/icons.rs
```
Expected:
- `lib.rs` が pass 2 完了時より小さい
- 結果をレビュー依頼に添える

次候補の判断ルール:
- app state / update loop が最大なら → `app.rs`
- domain/helper の塊が最大なら → その責務に応じた別モジュール

- [ ] **Step 7: reviewer に pass 3 のレビューを依頼する**

レビュー対象:
- `apps/egui-web/src/lib.rs`
- `apps/egui-web/src/render.rs`
- fresh verification evidence
- symbol move / LOC

レビュー観点:
- 純粋抽出を守れているか
- visibility が最小限に収まっているか
- `update` / wasm export が `lib.rs` に残っているか
- pass 4 候補が `app.rs` かどうか

- [ ] **Step 8: review 指摘があれば最小修正して commit し、結果をまとめる**

もし reviewer 指摘があれば、必要最小限だけ修正して focused な commit を切る。
その後、次をまとめる:
- `render.rs` に移したもの
- `lib.rs` に残したもの
- 新しい LOC
- 推奨される pass 4 候補
- fresh verification 結果
