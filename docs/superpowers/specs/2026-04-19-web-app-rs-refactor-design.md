# web `app.rs` 分割設計（第4パス）

## 背景
- `apps/web/src/lib.rs` は pass 1（`layout.rs` / `icons.rs`）、pass 2（`gpu.rs`）、pass 3（`render.rs`）の抽出後でも 906 LOC あり、まだ複数の責務を抱えている。
- pass 3 完了後の最終レビューでは、残存責務の中で最もまとまりがあり、次に切り出す自然な候補として **`QniApp` の状態管理 / input / update loop** が推奨された。
- 現在の `lib.rs` には、主に次の 2 系統が残っている。
  - `QniApp` 本体と、その内部状態 (`PlacedGate`, `DragState`)、input / drag / recompute / `eframe::App` 実装
  - gate/domain helper、共有定数、`Colors`、wasm export (`start`, `read_state_vector`)
- ユーザーはこの pass 4 について、次を選択した。
  - **A: 純粋抽出のみ**
  - **A1: `QniApp` 専用内部データ型 (`PlacedGate`, `DragState`) も `app.rs` に寄せる**
  - **B: `lib.rs` は中間段階のままでよく、薄い入口ファイル化までは今回求めない**
- したがって今回は、`QniApp` 責務塊を安全に `app.rs` へ移すことを優先し、挙動変更や domain/helper の積極再編は行わない。

## 目的
- `lib.rs` から `QniApp` の状態管理 / input / update loop を `apps/web/src/app.rs` に安全に分離する。
- 挙動変更なしの code motion に徹し、`lib.rs` から「アプリ本体の制御フロー」を追い出して責務境界をさらに見やすくする。
- `render.rs` / `gpu.rs` / `layout.rs` / `icons.rs` に続く第4段階として、将来の domain/helper 整理判断をしやすくする。

## 非目的
- `Colors` の別モジュール化。
- `GateKind` / `GateMatrix` / `GateParams` の再配置。
- `gate_matrix` / `gate_params` / `gate_params_controlled` の責務移動。
- wasm export (`start`, `read_state_vector`) の移動や公開面変更。
- `render.rs` / `gpu.rs` の責務再編。
- drag / drop / recompute / state panel / WebGPU 初期化の挙動変更。
- この pass だけで `lib.rs` を最終形まで薄くすること。
- この pass だけで全ファイルを 500 LOC 以下にすること。

## 採用方針
第4パスでは **`QniApp` に閉じた責務のみを `app.rs` に移す**。ユーザー選択どおり、純粋抽出のみを採用し、次のような「ついで変更」は行わない。

- `Colors` の移動
- gate/domain helper の再編
- wasm export の再配置
- render / gpu 呼び出しの責務変更
- input / drag / drop 制御の再設計
- `QniApp` field 構成の整理や rename
- helper 引数の整理や API の美化

これにより、今回の pass は「`app.rs` が app state / input / update を持ち、`lib.rs` は helper / 共有型 / wasm export を持つ」という責務線を作ることに限定する。

## 比較した案

### 案A: 純粋抽出のみ（採用）
- `app.rs` を追加し、`PlacedGate` / `DragState` / `QniApp` / `impl QniApp` / `impl eframe::App for QniApp` をそのまま移す。
- `lib.rs` には gate/domain helper、共有定数、`Colors`、wasm export を残す。
- 利点:
  - 変更リスクが最小。
  - pass 1〜3 と同じ「挙動変更なし」の進め方を維持できる。
  - `lib.rs` の責務境界がさらに見えやすくなる。
- 欠点:
  - `app.rs` は `crate::...` 経由で既存 helper / 定数 / `Colors` をかなり参照する中間段階になる。
  - 最終形としてはまだ domain/helper の整理が残る。

### 案B: 純粋抽出 + `Colors` を app 側へ移す
- 案Aに加えて `Colors` も `app.rs` に寄せる。
- 利点:
  - app loop 周辺の補助型を app 側に寄せられる。
  - `lib.rs` をやや薄くできる。
- 欠点:
  - `render.rs` / `gpu.rs` も `Colors` を使っており、依存整理が増える。
  - 純粋抽出より一段リスクが上がる。

### 案C: `app.rs` と同時に helper まで再編する
- 案Aに加えて gate/domain helper や共有型の再配置も進める。
- 利点:
  - `lib.rs` を大きく薄くできる。
- 欠点:
  - ユーザーの B 選択（中間段階でよい）と合わない。
  - 変更量が増え、失敗時の切り分けが難しい。

## 第4パスのモジュール境界

### 新規追加: `apps/web/src/app.rs`
ここには、**アプリ本体の状態管理 / input / update loop に閉じた責務**を移す。

対象:
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

意図:
- `QniApp` の状態と制御フローを 1 モジュールに閉じ込める。
- `lib.rs` から app loop の詳細を追い出し、共有型 / helper / wasm export を見やすくする。
- 既存の `render.rs` / `gpu.rs` を使う app モジュールとして責務を整理する。

### `apps/web/src/lib.rs` に残すもの
対象:
- gate/domain helper
  - `GateKind`
  - `GateMatrix`
  - `GateParams`
  - `gate_matrix`
  - `gate_params`
  - `gate_params_controlled`
- 共有 helper
  - `now_seconds`
  - `display_index_to_state_index`
  - `amplitude_qubits`
  - `color_rgba`
- 共有定数
- `Colors`
- wasm export の公開面 (`start`, `read_state_vector`)

意図:
- `lib.rs` には「共有型・補助関数・wasm の入口」を残す。
- 今回は `lib.rs` の薄い最終形を目指さず、中間段階として許容する。

## 依存関係の方針
この pass の主目的は **`lib.rs` から `app.rs` へ `QniApp` 責務塊を移すこと**である。ただし純粋抽出を成立させるため、pass 4 では一時的な相互参照を許容する。

想定する中間段階の依存:
- `lib.rs` → `app.rs`（`start(...)` から `app::QniApp::new(...)` を呼ぶ）
- `app.rs` → `crate::...` helper / 定数 / 既存 submodule
- `render.rs` / `layout.rs` → `crate::app::{QniApp, PlacedGate}`

方針:
- 今回は **依存の完全整理より、安全な抽出を優先** する。
- `QniApp` / `PlacedGate` を使う既存 sibling module は、root re-export ではなく **`crate::app::{...}` を明示 import する方針を採用** する。
- 新たに業務ロジックを `app.rs` に増やさず、既存 `QniApp` 本体が必要とする参照だけを持ち込む。
- `app.rs` から `render.rs` / `gpu.rs` / `layout.rs` を使う構造は、この pass では許容する。

### 許可する `app.rs` → `crate::...` 参照の種類
- app/domain 側シンボル
  - `crate::GateKind`
  - `crate::GateParams`
  - `crate::Colors`
  - `crate::gate_params`
  - `crate::gate_params_controlled`
  - `crate::now_seconds`
  - `crate::amplitude_qubits`
- 既存 submodule
  - `crate::render`
  - `crate::gpu`
  - `crate::layout`
- 既存 UI / drag / qubit / layout 定数
  - `MIN_QUBITS`
  - `MAX_QUBITS`
  - `MAX_STATE_COUNT`
  - `GATE_SIZE`
  - `PALETTE_SIZE`
  - `PALETTE_GAP`
  - `PALETTE_ROW_Y`
  - `LINE_Y`
  - `LINE_GAP`
  - `LINE_LEFT_OFFSET`
  - `LINE_RIGHT_OFFSET`
  - `CIRCUIT_PADDING`
  - `SNAP_DISTANCE`
  - `DRAG_REPAINT_*`
  - その他、`QniApp` 実装が既に参照している app loop 用定数

## 可視性の方針
- `QniApp` は `lib.rs` の `start(...)` から `app::QniApp::new(...)` を呼べるよう、**必要最小限で `pub(crate)`** とすることを許容する。
- `QniApp::new(...)` も `lib.rs` から呼べる最小限の visibility を持たせる。想定は **`pub(crate)`**。
- `PlacedGate` は `render.rs` と `layout.rs` から型参照され、かつ field を直接読まれているため、**型は `pub(crate)`、field は cross-module access に必要な最小限だけ `pub(crate)`** とする。
  - 少なくとも `id`, `kind`, `pos`, `wire` は `render.rs` / `layout.rs` から直接読まれる前提で維持する。
- `DragState` は原則 app 内部で閉じるが、もし sibling module から参照が必要ならその時点で最小限だけ `pub(crate)` を使う。
- `QniApp` の field も、`render.rs` から直接読まれるものだけ最小限で見せる。
  - 少なくとも `placed_gates`, `hovered_gate_id`, `hovered_palette_index`, `state_panel_offset`, `state_instance_cache` は render 側 call site を維持できる visibility を確保する。
  - 追加公開は禁止し、上記以外の field は可能なら `app.rs` 内に閉じる。
- `render.rs` から呼ばれる `QniApp` method も最小限の visibility を持たせる。
  - 少なくとも `collect_gate_params(...)` は `render.rs` 内 `draw_state_vector(...)` から呼ばれるため、**`pub(crate)`** にする必要がある。
  - 他に sibling module から参照される method があれば同じ原則で最小限だけ `pub(crate)` へ widen する。
- 外部公開 (`pub`) は追加しない。必要なら **`pub(crate)` に統一** し、`pub(super)` は新規導入しない。
- 既存 `render.rs` の visibility は、`app.rs` 抽出に必要な場合のみ最小限で調整する。`render.rs` の責務は変えない。
- **新しい外部 `pub` API を増やさない**。この pass で移す型 (`QniApp`, `PlacedGate`, `DragState`) は crate 内 visibility に留める。

## `start` / `read_state_vector` の扱い
wasm export の公開面は不必要に動かさない。

採用方針:
- `#[wasm_bindgen] pub async fn start(...)` は `lib.rs` に残す。
- `start(...)` は `app::QniApp::new(...)` を呼ぶ薄いエントリポイントに保つ。
- `#[wasm_bindgen] pub async fn read_state_vector(...)` は `lib.rs` に残す。
- `read_state_vector(...)` の責務や公開位置は変えない。

意図:
- wasm export の位置変更による余計なリスクを避ける。
- 一方で app 本体の実装詳細は `app.rs` に閉じる。

## 実装ガードレール
この pass は挙動変更なしの抽出に限定し、以下を原則とする。

- 移動対象の struct / impl / method 本体は、import 解決と最小限の可視性調整を除いて原則そのまま移す。
- 可視性変更は **必要最小限** に限定し、外部公開 (`pub`) は追加しない。
- `QniApp` の外部から見える挙動は変更しない。
- `render.rs` / `layout.rs` の import は、`crate::QniApp` / `crate::PlacedGate` から **`crate::app::{QniApp, PlacedGate}` への明示切り替え**を行う。root re-export は追加しない。
- drag / drop / recompute / state panel / startup repaint の挙動は変更しない。
- `render.rs` / `gpu.rs` の責務は変更しない。
- `Colors` はこの pass では `lib.rs` に残す。
- gate/domain helper はこの pass では `lib.rs` に残す。
- wasm export (`start`, `read_state_vector`) の関数名・シグネチャ・公開位置は維持する。
- 命名変更、最適化、コメント整理などの「ついで変更」はしない。

## 実装手順
1. `apps/web/src/app.rs` を追加し、`lib.rs` に `mod app;` を宣言する。
2. `PlacedGate`, `DragState`, `QniApp` を `app.rs` に移す。
3. `impl QniApp` を `app.rs` に移す。
4. `impl eframe::App for QniApp` を `app.rs` に移す。
5. `lib.rs` 側で `app::QniApp` import と `start(...)` の呼び出しを最小限調整する。
6. `render.rs` / `layout.rs` の import を `crate::app::{QniApp, PlacedGate}` 前提に最小限更新し、必要な field / method visibility だけを調整する。
7. `cargo check --target wasm32-unknown-unknown` を早い段階で実行して import / visibility 崩れを切り分ける。
8. 既存 Playwright / trunk build / diff check を再実行し、挙動差分がないことを確認する。
9. 最後に symbol move / remains check / tests unchanged / LOC / public-surface check を行い、pass 5 候補を見直す。

## 受け入れ条件
- `apps/web/src/app.rs` が追加されている。
- `lib.rs` から以下の定義が `app.rs` へ移っている。
  - `PlacedGate`
  - `DragState`
  - `QniApp`
  - `impl QniApp`
  - `impl eframe::App for QniApp`
- `lib.rs` には上記の定義が残っていない。
- `Colors` / gate/domain helper / wasm export は `lib.rs` に残っている。
- `cargo check --target wasm32-unknown-unknown` が通る。
- `pnpm exec playwright test` が通る。
- `trunk build` が通る。
- `git diff --check` が通る。
- `apps/web/tests/web.spec.js` は未変更である。
- `lib.rs` の LOC が pass 3 完了時より減っている。

## 検証
第4パス実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

この pass の web 検証系には `cargo insta` ベースの snapshot suite は含まれていないため、snapshot 確認は追加しない。UI 回帰は既存 Playwright を正本とする。
また、この pass の spec では CI 相当の repo-wide aggregate check（例: `./scripts/check-all.sh`）は **スコープ外** とする。理由は、今回の変更対象が `apps/web` の局所的な code motion であり、pass 1〜3 と同様に wasm cargo check + Playwright + trunk build + diff check を正本の受け入れ条件とするためである。

追加で、app symbol move を機械的に確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'struct PlacedGate|struct DragState|struct QniApp|impl QniApp|impl eframe::App for QniApp' apps/web/src/lib.rs apps/web/src/app.rs
```

期待値:
- 列挙した app シンボルは `app.rs` 側にある
- `lib.rs` にはそれらの定義が残っていない
- `impl` の確認は method-safe な文字列一致で行う

追加で、`render.rs` / `layout.rs` の import 更新が入っており、旧 root import が残っていないことを確認する。

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
    if need['need_qniapp'] and not re.search(r'use\s+crate::app::\{[^}]*\bQniApp\b[^}]*\}', text, re.S):
        raise SystemExit(f'missing crate::app import for QniApp in {path}')
    if need['need_placedgate'] and not re.search(r'use\s+crate::app::\{[^}]*\bPlacedGate\b[^}]*\}', text, re.S):
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

期待値:
- `render.rs` は `QniApp` / `PlacedGate` を `crate::app` から import している
- `layout.rs` は `PlacedGate` を `crate::app` から import している
- multi-line import を含め、旧 root import / root path 参照は残っていない

追加で、`lib.rs` に残すべきシンボルが残っていることを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'enum GateKind|struct GateMatrix|struct GateParams|fn gate_matrix|fn gate_params\(|fn gate_params_controlled|struct Colors|pub async fn start|pub async fn read_state_vector' apps/web/src/lib.rs apps/web/src/app.rs
```

期待値:
- 列挙した gate/domain helper、`Colors`、wasm export は `lib.rs` のみにある
- `app.rs` 側にはそれらの定義が増えていない

追加で、新しい外部 `pub` API や root re-export を増やしていないことを確認する。

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
    for m in re.finditer(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+((self::)?app::|crate::app::)', text, re.M):
        raise SystemExit(f'forbidden app re-export in {path}: {m.group(0)}')
    for m in re.finditer(r'^pub\s+(?!async fn start\b)(?!async fn read_state_vector\b)', text, re.M):
        raise SystemExit(f'unexpected external pub surface in {path}: {m.group(0)}')
print('ok')
PY
```

期待値:
- `start` / `read_state_vector` を除き、新しい外部 `pub` が増えていない
- `pub use app::...` だけでなく `pub use self::app::...`, `pub(crate) use app::...`, `pub(crate) use crate::app::...`, `pub(super) use ...` も追加されていない

追加で、widen した `pub(crate)` が承認した最小限に収まっていることを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re

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

期待値:
- `pub(crate)` 化は承認済みの最小限に収まっている
- 追加の field / method / re-export widening は発生していない

また、テストファイルが変更されていないことも確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/web/tests/web.spec.js
```

期待値:
- no output

LOC 確認として以下も実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/web/src/lib.rs apps/web/src/app.rs apps/web/src/render.rs apps/web/src/gpu.rs apps/web/src/layout.rs apps/web/src/icons.rs
```

## 第5パスの判断基準
この pass 4 の完了後、残る大きな責務は主に次の 2 つに寄る想定である。

- gate/domain helper と共有型
- wasm export と crate root の配線

したがって、pass 5 候補は次のルールで決める。

- helper / shared type の塊が最大なら → その責務に応じた shared/domain モジュール
- `lib.rs` の入口整理が最大なら → crate root の薄化を目的にした最終整理 pass

現時点の見立てでは、pass 4 後の `lib.rs` はまだ中間段階であり、次パスで薄い入口ファイル化を検討できる。