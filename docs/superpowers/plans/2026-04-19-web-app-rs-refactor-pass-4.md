# web `app.rs` 分割（第4パス）実装計画

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/web/src/lib.rs` から `QniApp` とその app-specific internal types / input / update loop を `apps/web/src/app.rs` に純粋抽出し、挙動を変えずに `lib.rs` をさらに縮小する。

**Architecture:** この pass は code motion 中心の純粋抽出のみを行う。`PlacedGate` / `DragState` / `QniApp` / `impl QniApp` / `impl eframe::App for QniApp` を `app.rs` へ寄せ、`lib.rs` には gate/domain helper・共有定数・`Colors`・wasm export を残す。`render.rs` と `layout.rs` は `crate::app::{QniApp, PlacedGate}` を明示 import する中間段階とし、root re-export は追加しない。

**Tech Stack:** Rust, eframe/egui, egui_wgpu, wgpu/WebGPU, wasm-bindgen, Playwright, trunk

---

## この pass のファイル構成

- Create: `apps/web/src/app.rs`
  - `PlacedGate`
  - `DragState`
  - `QniApp`
  - `impl QniApp`
    - `new`
    - `layout_qubits`
    - `state_qubits`
    - `update_qubit_count`
    - `state_count`
    - `collect_gate_params`
    - `handle_input`
    - `schedule_drag_repaint`
  - `impl eframe::App for QniApp`
- Modify: `apps/web/src/lib.rs`
  - `mod app;` を追加する
  - `start(...)` が `app::QniApp::new(...)` を呼ぶように保つ
  - gate/domain helper / `Colors` / wasm export を残す
- Modify: `apps/web/src/render.rs`
  - `QniApp` / `PlacedGate` の import を `crate::app::{...}` に切り替える
  - `collect_gate_params(...)` や `QniApp` field への access が成立するよう最小限の visibility 前提に合わせる
- Modify: `apps/web/src/layout.rs`
  - `PlacedGate` の import を `crate::app::PlacedGate` に切り替える
- Verify: `apps/web/tests/web.spec.js`
  - この pass では変更しない
- Reference spec: `docs/superpowers/specs/2026-04-19-web-app-rs-refactor-design.md`

## ガードレール

- `cargo check` は **必ず** `--target wasm32-unknown-unknown` で実行する。
- この pass は **pure extraction only**。drag / drop / recompute / state panel / startup repaint / WebGPU 初期化の挙動は変更しない。
- `Colors` は `lib.rs` に残す。
- gate/domain helper (`GateKind`, `GateMatrix`, `GateParams`, `gate_matrix`, `gate_params`, `gate_params_controlled`) は `lib.rs` に残す。
- wasm export (`start`, `read_state_vector`) は `lib.rs` に残す。
- `render.rs` / `gpu.rs` の責務は変更しない。
- `apps/web/tests/web.spec.js` は変更しない。
- 可視性変更は必要最小限に限定し、外部公開 (`pub`) は追加しない。crate 内可視性が必要な場合は **`pub(crate)` に統一** する。
- `PlacedGate` は `render.rs` / `layout.rs` から直接読まれる field (`id`, `kind`, `pos`, `wire`) だけ `pub(crate)` を許容する。
- `QniApp` field は `render.rs` から直接読まれるもの (`placed_gates`, `hovered_gate_id`, `hovered_palette_index`, `state_panel_offset`, `state_instance_cache`) だけ最小限で `pub(crate)` を許容する。
- `collect_gate_params(...)` は `render.rs` から呼ばれるため、必要最小限で `pub(crate)` を許容する。
- `QniApp::new(...)` は `lib.rs` の `start(...)` から呼ぶため、必要最小限で `pub(crate)` を許容する。
- `pub use app::...` / `pub(crate) use app::...` / `pub(super) use app::...` のような root re-export は追加しない。
- repo-wide aggregate check（例: `./scripts/check-all.sh`）はこの pass ではスコープ外とし、受け入れ条件は web の wasm cargo check + Playwright + trunk build + diff check を正本とする。

### Task 1: `app.rs` へ `QniApp` 責務塊を原子的に抽出する

**Files:**

- Create: `apps/web/src/app.rs`
- Modify: `apps/web/src/lib.rs`
- Modify: `apps/web/src/render.rs`
- Modify: `apps/web/src/layout.rs`
- Reference: `docs/superpowers/specs/2026-04-19-web-app-rs-refactor-design.md`

- [ ] **Step 1: 抽出前のベースラインを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'web canvas renders content|dragging does not grow state vector until drop|placed circuit gate keeps its visual while dragging another gate|default chromium shows a visible WebGPU error instead of a blank page'
```

Expected:

- `cargo check --target wasm32-unknown-unknown` が success
- grep で一致した Playwright がすべて pass

- [ ] **Step 2: `app.rs` を作成し、`QniApp` 責務塊を一度に移す**

`apps/web/src/app.rs` を作成し、`apps/web/src/lib.rs` に `mod app;` を追加したうえで、次を **同じ編集で原子的に** 移す。

- `PlacedGate`
- `DragState`
- `QniApp`
- `impl QniApp`
  - `new`
  - `layout_qubits`
  - `state_qubits`
  - `update_qubit_count`
  - `state_count`
  - `collect_gate_params`
  - `handle_input`
  - `schedule_drag_repaint`
- `impl eframe::App for QniApp`

方針:

- `QniApp` struct だけ先に動かして中間コンパイルを取ろうとしない
- `impl QniApp` や `impl eframe::App` が親 module に残る一時状態は privacy 破綻を起こすため、**struct + impl 群を一括で移す**
- 本体は import 解決と最小限の visibility 解決以外そのまま移す

- [ ] **Step 3: `lib.rs` / `render.rs` / `layout.rs` の接続を最小限調整する**

更新対象:

- `apps/web/src/lib.rs`
- `apps/web/src/render.rs`
- `apps/web/src/layout.rs`

確認観点:

- `lib.rs` は `app::QniApp` を import し、`start(...)` から `app::QniApp::new(...)` を呼べること
- `render.rs` は `QniApp` / `PlacedGate` を `crate::app::{QniApp, PlacedGate}` から明示 import すること
- `layout.rs` は `PlacedGate` を `crate::app::PlacedGate` から明示 import すること
- root re-export は追加しないこと

- [ ] **Step 4: visibility を承認済み最小限に調整する**

方針:

- crate 内可視性が必要な場合は **`pub(crate)` に統一** する
- `PlacedGate` は `id` / `kind` / `pos` / `wire` だけ `pub(crate)` を許容する
- `QniApp` field は `placed_gates`, `hovered_gate_id`, `hovered_palette_index`, `state_panel_offset`, `state_instance_cache` だけ `pub(crate)` を許容する
- `QniApp::new(...)` と `collect_gate_params(...)` だけ `pub(crate)` を許容する
- それ以外の widening は禁止する
- `render.rs` の visibility は import 解決以外で widen しない

- [ ] **Step 5: 原子的抽出後にコンパイルする**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
```

Expected:

- module / import / visibility / `start(...)` / `impl eframe::App` 周りの error がなく success

- [ ] **Step 6: app loop 抽出後の focused 回帰を回す**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'dragging does not grow state vector until drop|dragged x gate keeps the same visual as after drop|placed circuit gate keeps its visual while dragging another gate|H on q0 and q1 yields uniform superposition'
```

Expected:

- 一致したテストがすべて pass

- [ ] **Step 7: app モジュール抽出を commit する**

Run:

```bash
git add /home/yasuhito/Work/qni-webgpu/apps/web/src/app.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/render.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/layout.rs
git commit -m "refactor: extract web app module"
```

### Task 2: pass 4 全体を検証し、次の分割候補を決める

**Files:**

- Modify: none required unless review で修正が入る
- Reference: `apps/web/src/lib.rs`
- Reference: `apps/web/src/app.rs`
- Reference: `apps/web/src/render.rs`
- Reference: `apps/web/src/layout.rs`
- Reference spec: `docs/superpowers/specs/2026-04-19-web-app-rs-refactor-design.md`

- [ ] **Step 1: pass 4 の完全検証を実行する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && env PATH="$HOME/.cargo/bin:$PATH" cargo check --target wasm32-unknown-unknown
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

- [ ] **Step 3: app シンボルが意図どおり移動したことを機械的に確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'struct PlacedGate|struct DragState|struct QniApp|impl QniApp|impl eframe::App for QniApp' apps/web/src/lib.rs apps/web/src/app.rs
```

Expected:

- 列挙した app シンボルは `app.rs` 側にある
- `lib.rs` にはそれらの定義が残っていない

- [ ] **Step 4: `render.rs` / `layout.rs` の import 更新と旧 root import 不在を確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re

checks = {
    'apps/web/src/render.rs': {'need_qniapp': True, 'need_placedgate': True},
    'apps/web/src/layout.rs': {'need_qniapp': False, 'need_placedgate': True},
}
for path, need in checks.items():
    text = Path(path).read_text()
    if need['need_qniapp'] and not (
        re.search(r'use\s+crate::app::\{[^}]*\bQniApp\b[^}]*\}', text, re.S)
        or re.search(r'use\s+crate::app::QniApp\s*;', text)
    ):
        raise SystemExit(f'missing crate::app import for QniApp in {path}')
    if need['need_placedgate'] and not (
        re.search(r'use\s+crate::app::\{[^}]*\bPlacedGate\b[^}]*\}', text, re.S)
        or re.search(r'use\s+crate::app::PlacedGate\s*;', text)
    ):
        raise SystemExit(f'missing crate::app import for PlacedGate in {path}')
    if re.search(r'use\s+crate::\{[^}]*\bQniApp\b[^}]*\}', text, re.S):
        raise SystemExit(f'old root import for QniApp remains in {path}')
    if re.search(r'use\s+crate::\{[^}]*\bPlacedGate\b[^}]*\}', text, re.S):
        raise SystemExit(f'old root import for PlacedGate remains in {path}')
    if re.search(r'crate::QniApp\b', text):
        raise SystemExit(f'root path reference crate::QniApp remains in {path}')
    if re.search(r'crate::PlacedGate\b', text):
        raise SystemExit(f'root path reference crate::PlacedGate remains in {path}')
print('ok')
PY
```

Expected:

- `render.rs` は `QniApp` / `PlacedGate` を `crate::app` から import している
- `layout.rs` は `PlacedGate` を `crate::app` から import している
- multi-line import を含め、旧 root import / root path 参照は残っていない

- [ ] **Step 5: `lib.rs` に残すべき helper / export が残っていることを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'enum GateKind|struct GateMatrix|struct GateParams|fn gate_matrix|fn gate_params\(|fn gate_params_controlled|struct Colors|pub async fn start|pub async fn read_state_vector' apps/web/src/lib.rs apps/web/src/app.rs
```

Expected:

- 列挙した gate/domain helper、`Colors`、wasm export は `lib.rs` のみにある
- `app.rs` 側にはそれらの定義が増えていない

- [ ] **Step 6: 新しい外部 `pub` API や root re-export を増やしていないことを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re

files = [
    'apps/web/src/lib.rs',
    'apps/web/src/app.rs',
    'apps/web/src/render.rs',
    'apps/web/src/layout.rs',
]
for path in files:
    text = Path(path).read_text()
    for m in re.finditer(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+((self::)?app::|crate::app::|crate::\{[^}]*app::)', text, re.M):
        raise SystemExit(f'forbidden app re-export in {path}: {m.group(0)}')
    for m in re.finditer(r'^pub\s+(?!async fn start\b)(?!async fn read_state_vector\b)', text, re.M):
        raise SystemExit(f'unexpected external pub surface in {path}: {m.group(0)}')
print('ok')
PY
```

Expected:

- `start` / `read_state_vector` を除き、新しい外部 `pub` が増えていない
- `pub use app::...` だけでなく `pub(crate) use app::...` / `pub(super) use app::...` も追加されていない

- [ ] **Step 7: widen した `pub(crate)` が承認した最小限に収まっていることを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path

text = Path('apps/web/src/app.rs').read_text().splitlines()
allowed = {
    'pub(crate) struct PlacedGate',
    'pub(crate) id:',
    'pub(crate) kind:',
    'pub(crate) pos:',
    'pub(crate) wire:',
    'pub(crate) struct QniApp',
    'pub(crate) placed_gates:',
    'pub(crate) hovered_gate_id:',
    'pub(crate) hovered_palette_index:',
    'pub(crate) state_panel_offset:',
    'pub(crate) state_instance_cache:',
    'pub(crate) fn new',
    'pub(crate) fn collect_gate_params',
}
for line in text:
    s = line.strip()
    if not s.startswith('pub(crate)'):
        continue
    if not any(s.startswith(prefix) for prefix in allowed):
        raise SystemExit(f'unapproved pub(crate) widening: {s}')
print('ok')
PY
```

Expected:

- `pub(crate)` 化は承認済みの最小限に収まっている
- 追加の field / method / re-export widening は発生していない

- [ ] **Step 8: `render.rs` に新しい visibility widening が入っていないことを commit diff で確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && git show --format= --unified=0 HEAD -- apps/web/src/render.rs | rg '^[+-].*pub\(crate\)|^[+-].*pub\(super\)'
```

Expected:

- no output
- 直前の抽出 commit に `render.rs` の新しい visibility widening が含まれていない
- `render.rs` の変更は import 調整に留まっている

- [ ] **Step 9: テストファイルが変更されていないことを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/web/tests apps/web/test-node
```

Expected:

- no output

- [ ] **Step 10: LOC を測り、次の候補を決める**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/web/src/lib.rs apps/web/src/app.rs apps/web/src/render.rs apps/web/src/gpu.rs apps/web/src/layout.rs apps/web/src/icons.rs
```

Expected:

- `lib.rs` が pass 3 完了時より小さい
- 結果をレビュー依頼に添える

次候補の判断ルール:

- helper / shared type の塊が最大なら → その責務に応じた shared/domain モジュール
- `lib.rs` の入口整理が最大なら → crate root の薄化を目的にした最終整理 pass

- [ ] **Step 11: reviewer に pass 4 のレビューを依頼する**

レビュー対象:

- `apps/web/src/lib.rs`
- `apps/web/src/app.rs`
- `apps/web/src/render.rs`
- `apps/web/src/layout.rs`
- fresh verification evidence
- symbol move / remains / public-surface / LOC

レビュー観点:

- 純粋抽出を守れているか
- visibility が最小限に収まっているか
- `Colors` / gate helper / wasm export が `lib.rs` に残っているか
- root re-export が追加されていないか
- 次候補が helper/shared 側か crate root 整理か

- [ ] **Step 12: review 指摘があれば最小修正して commit し、結果をまとめる**

もし reviewer 指摘があれば、必要最小限だけ修正して focused な commit を切る。
その後、次をまとめる:

- `app.rs` に移したもの
- `lib.rs` に残したもの
- 新しい LOC
- 推奨される次パス候補
- fresh verification 結果
