# web `gpu.rs` 分割（第2パス）実装計画

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/web/src/lib.rs` から GPU / shader / readback の責務を `apps/web/src/gpu.rs` に純粋抽出し、挙動を変えずに `lib.rs` をさらに縮小する。

**Architecture:** この pass は code motion 中心の純粋抽出のみを行う。`QniApp`、input / drag、palette / circuit / state panel 描画、gate parameter generation、wasm export の公開面は `lib.rs` に残し、shader・pipeline/resource・egui_wgpu callback・readback 実装本体だけを `gpu.rs` に寄せる。

**Tech Stack:** Rust, eframe/egui, egui_wgpu, wgpu/WebGPU, wasm-bindgen, Playwright, trunk

---

## この pass のファイル構成

- Create: `apps/web/src/gpu.rs`
  - `StateInstance`
  - `RenderParams`
  - `RenderColors`
  - `STATE_WORKGROUP_SIZE`
  - `STATE_COMPUTE_SHADER`
  - `STATE_RENDER_SHADER`
  - `StateVectorResources`
  - `StateVectorCallback`
  - `GpuReadbackState`
  - `GPU_READBACK`
  - `read_state_vector` の内部実装本体
- Modify: `apps/web/src/lib.rs`
  - `mod gpu;` を追加する
  - `gpu.rs` へ移した型・関数を import する
  - `StateInstanceCache` が `gpu::StateInstance` を参照するようにする
  - `#[wasm_bindgen] pub async fn read_state_vector()` は薄い wrapper として残す
- Verify: `apps/web/tests/web.spec.js`
  - この pass では変更しない
- Reference spec: `docs/superpowers/specs/2026-04-15-web-gpu-rs-refactor-design.md`

## ガードレール

- `cargo check` は **必ず** `--target wasm32-unknown-unknown` で実行する。
- 既存 shader 文字列、buffer / bind group / pipeline 初期化順、callback の制御フローは変更しない。
- `start` と `read_state_vector` の wasm export 名・シグネチャ・公開位置は維持する。
- 可視性変更は必要最小限に限定し、`pub` は増やさない。必要なら `pub(crate)` までに留める。
- `gpu.rs` が新たに参照してよい親シンボルは `crate::Colors`、`crate::GateParams`、`crate::MAX_STATE_COUNT` のみ。
- `GateParams` / `gate_params` / `gate_params_controlled` / `gate_matrix` / `collect_gate_params` は `lib.rs` に残す。
- `QniApp` / drag / palette / circuit / state panel / state cache は `lib.rs` に残す。
- テスト追加や挙動変更はこの pass では行わない。既存 Playwright を安全網として使う。

### Task 1: GPU モジュールの土台と resource 層を抽出する

**Files:**

- Create: `apps/web/src/gpu.rs`
- Modify: `apps/web/src/lib.rs`
- Reference: `docs/superpowers/specs/2026-04-15-web-gpu-rs-refactor-design.md`

- [ ] **Step 1: 抽出前のベースラインを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'web canvas renders content|H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns'
```

Expected:

- `cargo check --target wasm32-unknown-unknown` が success
- grep で一致した Playwright がすべて pass

- [ ] **Step 2: `gpu.rs` を作成してモジュール宣言を追加する**

`apps/web/src/gpu.rs` を作成し、`apps/web/src/lib.rs` の先頭付近に `mod gpu;` を追加する。

初期スケルトン例:

```rust
use eframe::egui;
use eframe::{egui_wgpu, wgpu};
use std::cell::RefCell;
use std::sync::Arc;
use wgpu::util::DeviceExt as _;

use crate::{Colors, GateParams, MAX_STATE_COUNT};
```

- [ ] **Step 3: GPU resource 層をそのまま移す**

`lib.rs` から以下を `gpu.rs` に移す。

- `StateInstance`
- `RenderParams`
- `RenderColors`
- `STATE_WORKGROUP_SIZE`
- `STATE_COMPUTE_SHADER`
- `STATE_RENDER_SHADER`
- `StateVectorResources`
- `impl StateVectorResources`

`lib.rs` 側では、必要な import だけに置き換える。

- [ ] **Step 4: 最小限の import / visibility を直してコンパイルする**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
```

Expected:

- unresolved module/symbol error がなく success

- [ ] **Step 5: resource 抽出後の focused 回帰を回す**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'web canvas renders content|H on q0 and q1 yields uniform superposition'
```

Expected:

- 一致したテストがすべて pass

- [ ] **Step 6: resource 抽出を commit する**

Run:

```bash
git add /home/yasuhito/Work/qni-webgpu/apps/web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/gpu.rs
git commit -m "refactor: extract web gpu resources"
```

### Task 2: egui_wgpu callback と readback 状態を一緒に抽出する

**Files:**

- Modify: `apps/web/src/gpu.rs`
- Modify: `apps/web/src/lib.rs`
- Verify: `apps/web/tests/web.spec.js`

- [ ] **Step 1: callback / readback state 抽出前に focused baseline を再確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns'
```

Expected:

- 一致したテストがすべて pass

- [ ] **Step 2: `StateVectorCallback` と readback state を一緒に `gpu.rs` に移す**

`lib.rs` から以下を `gpu.rs` に移す。

- `StateVectorCallback`
- `impl egui_wgpu::CallbackTrait for StateVectorCallback`
- `GpuReadbackState`
- `GPU_READBACK`

`prepare` / `paint` の本体と `GPU_READBACK` 更新タイミングは変更しない。

- [ ] **Step 3: `StateInstanceCache` と call site を `gpu::StateInstance` / `gpu::StateVectorCallback` に接続する**

`lib.rs` 側で以下を最小限調整する。

- `StateInstanceCache.instances: Arc<[gpu::StateInstance]>` になるよう import を直す
- state panel 描画で `gpu::RenderColors::new(&colors)` と `gpu::StateVectorCallback { ... }` を使うようにする

変更例のイメージ:

```rust
use crate::gpu::{RenderColors, StateInstance, StateVectorCallback};
```

- [ ] **Step 4: callback 抽出後にコンパイルする**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
```

Expected:

- callback trait / type visibility / import 周りの error がなく success

- [ ] **Step 5: state vector 計算系の focused 回帰を回す**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|Control does not affect gates in other columns'
```

Expected:

- 一致したテストがすべて pass

- [ ] **Step 6: callback 抽出を commit する**

Run:

```bash
git add /home/yasuhito/Work/qni-webgpu/apps/web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/gpu.rs
git commit -m "refactor: extract web gpu callback"
```

### Task 3: readback 実装本体と wasm wrapper を整理する

**Files:**

- Modify: `apps/web/src/gpu.rs`
- Modify: `apps/web/src/lib.rs`
- Verify: `apps/web/tests/web.spec.js`

- [ ] **Step 1: readback 依存テストの baseline を確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|default chromium shows a visible WebGPU error instead of a blank page'
```

Expected:

- 一致したテストがすべて pass

- [ ] **Step 2: readback 実装本体を `gpu.rs` に移し、`lib.rs` には薄い wrapper を残す**

wasm 専用 import は、readback 実装本体に必要な分だけ `gpu.rs` へ移す。

実装方針:

- `gpu.rs` に `pub(crate) async fn read_state_vector_impl() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue>` を作る
- 現在の `read_state_vector` 本体はそこへ移す
- `lib.rs` の `#[wasm_bindgen] pub async fn read_state_vector()` は次の形にする

```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn read_state_vector() -> Result<js_sys::Float32Array, wasm_bindgen::JsValue> {
    gpu::read_state_vector_impl().await
}
```

- [ ] **Step 3: wrapper 置換後に `lib.rs` 側から readback 実装詳細を消す**

確認観点:

- `lib.rs` には staging buffer / map_async / oneshot などの readback 実装詳細を残さない
- `lib.rs` 側には export wrapper のみを残す

- [ ] **Step 4: wasm export 互換を保ったままコンパイルする**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
```

Expected:

- `start` / `read_state_vector` の wasm 周りを含めて success

- [ ] **Step 5: readback を使う focused 回帰を再実行する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'H on q0 and q1 yields uniform superposition|CNOT with control on q1 yields bell state|default chromium shows a visible WebGPU error instead of a blank page'
```

Expected:

- 一致したテストがすべて pass

- [ ] **Step 6: readback 抽出を commit する**

Run:

```bash
git add /home/yasuhito/Work/qni-webgpu/apps/web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/gpu.rs
git commit -m "refactor: extract web gpu readback"
```

### Task 4: pass 2 全体を検証し、次の分割候補を決める

**Files:**

- Modify: none required unless review で修正が入る
- Reference: `apps/web/src/lib.rs`
- Reference: `apps/web/src/gpu.rs`
- Reference: `docs/superpowers/specs/2026-04-15-web-gpu-rs-refactor-design.md`

- [ ] **Step 1: pass 2 の完全検証を実行する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && trunk build
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

- [ ] **Step 3: GPU シンボルが意図どおり移動したことを機械的に確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n '^(pub\(crate\) )?(struct StateInstance|struct RenderParams|struct RenderColors|const STATE_WORKGROUP_SIZE|const STATE_COMPUTE_SHADER|const STATE_RENDER_SHADER|struct StateVectorResources|struct StateVectorCallback|struct GpuReadbackState)|GPU_READBACK' apps/web/src/lib.rs apps/web/src/gpu.rs
```

Expected:

- 列挙した GPU シンボルと `GPU_READBACK` は `gpu.rs` のみにある
- `lib.rs` にはそれらの定義が残っていない

- [ ] **Step 4: wrapper と wasm export の公開面を機械的に確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n '#\[wasm_bindgen\]|pub async fn start|pub async fn read_state_vector|read_state_vector_impl' apps/web/src/lib.rs apps/web/src/gpu.rs
```

Expected:

- `#[wasm_bindgen]`, `pub async fn start`, `pub async fn read_state_vector` は `lib.rs` のみにある
- `read_state_vector_impl` は `gpu.rs` 側にある

- [ ] **Step 5: `gpu.rs` の親参照が許可範囲内か確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'crate::' apps/web/src/gpu.rs
```

Expected:

- `crate::Colors`
- `crate::GateParams`
- `crate::MAX_STATE_COUNT`
のみが現れる

- [ ] **Step 6: テストファイルが変更されていないことを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/web/tests/web.spec.js
```

Expected:

- no output

- [ ] **Step 7: LOC を測り、次の候補を決める**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/web/src/lib.rs apps/web/src/gpu.rs apps/web/src/layout.rs apps/web/src/icons.rs
```

Expected:

- `lib.rs` が pass 1 完了時より小さい
- 結果をレビュー依頼に添える

次候補の判断ルール:

- 描画責務が最大なら → `render.rs`
- app/update/state glue が最大なら → `app.rs`

- [ ] **Step 8: reviewer に pass 2 のレビューを依頼する**

レビュー対象:

- `apps/web/src/lib.rs`
- `apps/web/src/gpu.rs`
- fresh verification evidence
- symbol move / wrapper check / LOC

レビュー観点:

- 純粋抽出を守れているか
- wasm API stability が保たれているか
- `gpu.rs` の依存が spec の許可範囲内か
- pass 3 候補が `render.rs` / `app.rs` のどちらか

- [ ] **Step 9: review 指摘があれば最小修正して commit し、結果をまとめる**

もし reviewer 指摘があれば、必要最小限だけ修正して focused な commit を切る。
その後、次をまとめる:

- `gpu.rs` に移したもの
- `lib.rs` に残したもの
- 新しい LOC
- 推奨される pass 3 候補
- fresh verification 結果
