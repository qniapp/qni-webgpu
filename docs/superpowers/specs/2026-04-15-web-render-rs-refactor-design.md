# web `render.rs` 分割設計（第3パス）

## 背景

- `apps/web/src/lib.rs` は pass 1（`layout.rs` / `icons.rs`）と pass 2（`gpu.rs`）の抽出後でも 1353 LOC あり、まだ責務が大きく残っている。
- pass 2 完了後の最終レビューでは、残存責務の中で最もまとまりがあり、次に切り出す自然な候補として **render.rs** が推奨された。
- 現在の `lib.rs` には、主に次の 2 系統が残っている。
  - `QniApp` の状態管理 / input / drag / update loop
  - circuit / palette / state panel の描画と、その周辺レイアウト・state instance cache
- このうち後者は、`gpu.rs`・`icons.rs`・`layout.rs` とすでに周辺モジュールが分かれており、**描画責務として一塊で外へ出しやすい**。
- ユーザーはこの pass 3 について、**A: 純粋抽出のみ** を選択した。したがって、今回は render helper 群を安全に `render.rs` へ移すことを優先し、挙動変更や境界の積極的整理は行わない。

## 目的

- `lib.rs` から描画と state panel 表示まわりの責務を `apps/web/src/render.rs` に安全に分離する。
- 挙動変更なしの code motion に徹し、`lib.rs` を「app state / input / update / wasm export」中心のファイルへ近づける。
- 将来の `app.rs` あるいはさらなる分割判断をしやすくする。

## 非目的

- `QniApp` 自体を `app.rs` に移すこと。
- input / drag / drop 制御の再設計。
- `Colors` の別モジュール化。
- `GateKind` / `PlacedGate` / `GateParams` などドメイン型の再配置。
- `gpu.rs` / `icons.rs` / `layout.rs` の責務再編。
- 描画ロジックの最適化や visual change。
- この pass だけで全ファイルを 500 LOC 以下にすること。

## 採用方針

第3パスでは **rendering に閉じた責務のみを `render.rs` に移す**。ユーザー選択どおり、純粋抽出のみを採用し、次のような「ついで変更」は行わない。

- `QniApp` field の再編
- `Colors` の別ファイル化
- `schedule_drag_repaint` のような update/timing helper の移動
- state panel drag のイベント処理位置変更
- palette / circuit / state panel の見た目変更
- render helper の命名変更や引数整理
- `gpu.rs` に渡すデータ生成責務の移動

これにより、今回の pass は「app が状態と入力を持ち、render モジュールが描画と表示用レイアウト/キャッシュを担当する」という責務線を作ることに限定する。

## 比較した案

### 案A: 純粋抽出のみ（採用）

- `render.rs` を追加し、circuit / palette / state panel の描画 helper と state panel 用 layout/cache 型をそのまま移す。
- `lib.rs` には `QniApp`、input/drag、update loop、gate parameter generation、wasm export を残す。
- 利点:
  - 変更リスクが最小。
  - pass 1 / pass 2 と同じ「挙動変更なし」の進め方を維持できる。
  - `lib.rs` の責務境界がかなり見えやすくなる。
- 欠点:
  - `render.rs` は `crate::QniApp` や複数の定数・helper に依存した中間段階になる。
  - 最終形としてはまだ `app.rs` との分離が残る。

### 案B: 純粋抽出 + 軽い境界整理

- 案Aに加えて helper visibility や import を積極的に整理する。
- 利点:
  - モジュール境界がやや明確になる。
- 欠点:
  - code motion だけではなくなり、レビュー時に挙動差分の混入を疑いやすい。
  - 今回のユーザー選択 A より変更面積が増える。

### 案C: render と app を同時再編

- `render.rs` 抽出に加えて `QniApp` や `update` も再配置する。
- 利点:
  - 一気に最終形へ近づける可能性がある。
- 欠点:
  - 変更量が大きく、失敗時の切り分けが難しい。
  - 今回の「純粋抽出のみ」方針と合わない。

## 第3パスのモジュール境界

### 新規追加: `apps/web/src/render.rs`

ここには、**描画そのものと、その表示を支える layout/cache 責務**を移す。

対象:

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

意図:

- circuit / palette / state panel の描画責務を 1 モジュールに閉じ込める。
- `lib.rs` から「どう描くか」の詳細を追い出し、`update` の流れを読みやすくする。
- `gpu.rs`・`icons.rs`・`layout.rs` を使う上位の描画モジュールとして整理する。

### `apps/web/src/lib.rs` に残すもの

対象:

- `QniApp`
- `handle_input`
- `schedule_drag_repaint`
- `state_count` / `layout_qubits` / `collect_gate_params` など app/domain 側 helper
- `GateKind` / `PlacedGate` / `DragState` / `GateParams` / `Colors`
- `gate_matrix` / `gate_params` / `gate_params_controlled`
- `eframe::App for QniApp` の `update`
- wasm export の公開面 (`start`, `read_state_vector`)

意図:

- `lib.rs` には「状態を持ち、入力を処理し、描画を呼び出す app の流れ」を残す。
- render モジュールに、入力制御や wasm 公開面まで背負わせない。

## 依存関係の方針

この pass で目指す主方向は **`lib.rs` → `render.rs`** である。ただし純粋抽出を成立させるため、pass 3 では `render.rs` から既存の親シンボルを参照することを許容する。

許可する依存の種類:

- app/domain 側シンボル
  - `crate::QniApp`
  - `crate::Colors`
  - `crate::GateKind`
  - `crate::PlacedGate`
  - `crate::should_use_fast_gate_body`
  - `crate::amplitude_qubits`
  - `crate::display_index_to_state_index`
- 既存 submodule
  - `crate::gpu`
  - `crate::icons`
  - `crate::layout`
- 既存 UI 定数
  - `GATE_SIZE`
  - `PALETTE_SIZE`
  - `PALETTE_GAP`
  - `PALETTE_ROW_Y`
  - `CIRCUIT_PADDING`
  - `STATE_CIRCLE_*`
  - `LINE_Y` / `LINE_GAP`
  - `REM`
  - `SNAP_DISTANCE`
  - その他、今回移動する描画本体が既に参照している描画定数

方針:

- 今回は **依存の完全整理より、安全な抽出を優先** する。
- 新たに業務ロジックを `render.rs` に増やさず、既存描画本体が必要とする参照だけを持ち込む。
- `render.rs` から `crate::gpu`, `crate::icons`, `crate::layout` を使う構造は、この pass では許容する。

## `update` の扱い

`eframe::App for QniApp` の `update` は、アプリの制御フローの中心なので今回は `lib.rs` に残す。

採用方針:

- `update` 本体は `lib.rs` に残す。
- その中で呼ばれる render helper だけを `render.rs` に移す。
- state panel drag ハンドル処理や recompute の意思決定も、`update` に残す。

意図:

- app loop と render helper を分離しつつ、公開/制御フローは動かさない。
- 次パスで `app.rs` を検討する余地を残す。

## 実装ガードレール

この pass は挙動変更なしの抽出に限定し、以下を原則とする。

- 移動対象の関数/impl/struct 本体は、import 解決と最小限の可視性調整を除いて原則そのまま移す。
- 可視性変更は **必要最小限** に限定し、外部公開 (`pub`) は追加しない。必要なら `pub(super)` または `pub(crate)` までとする。
- `impl QniApp` のうち `lib.rs` 側の `update` から呼ばれる render helper は、純粋抽出を保つため **`pub(super)` を基本** とする。
- `StatePanelLayout` と `StateInstanceCache` は `QniApp` の field / call site から参照できるよう、**必要最小限で `pub(super)`** とする。
- `StatePanelLayout` の field は、`lib.rs` 側で直接読むもの（少なくとも `state_rect` と `handle_height`）だけを `pub(super)` にし、それ以外は可能なら `render.rs` 内に閉じる。
- `StateInstanceKey` は `render.rs` 内で閉じられるなら private のままにする。
- `draw_circuit` / `draw_palette` / `draw_state_vector` の描画内容は変更しない。
- fast-drag の条件分岐は変更しない。
- palette が drag 中も通常描画を維持する現行挙動は変更しない。
- state panel の shadow / handle / clip rect / callback 生成順は変更しない。
- `StateInstanceCache` の key 構造や cache hit 条件は変更しない。
- `update` の制御フロー、state panel drag のイベント処理、recompute 判定は変更しない。
- `Colors` はこの pass では `lib.rs` に残す。
- 命名変更、最適化、コメント整理などの「ついで変更」はしない。

## 実装手順

1. `apps/web/src/render.rs` を追加し、`lib.rs` に `mod render;` を宣言する。
2. `StatePanelLayout`, `StateInstanceKey`, `StateInstanceCache` を `render.rs` に移す。
3. `circuit_content_height`, `draw_circuit`, `draw_palette` を `render.rs` へ移す。
4. `state_panel_layout`, `clamp_state_panel_offset`, `state_instances_for`, `draw_state_vector` を `render.rs` に移す。
5. `lib.rs` 側で `render::StateInstanceCache` など必要な型 import を最小限調整する。
6. `cargo check --target wasm32-unknown-unknown` を早い段階で実行して import / visibility 崩れを切り分ける。
7. drag / palette / state vector 系の既存 Playwright を再実行し、描画差分がないことを確認する。
8. 最後に full verification と symbol move / tests unchanged / LOC 確認を行い、pass 4 候補を見直す。

## 受け入れ条件

- `apps/web/src/render.rs` が追加されている。
- `lib.rs` から以下の定義が `render.rs` へ移っている。
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
- `lib.rs` には上記の定義が残っていない。
- `QniApp` / input / drag / update loop / wasm export / `Colors` / gate parameter generation は `lib.rs` に残っている。
- 既存の Playwright / build / wasm cargo check が通る。
- `apps/web/tests/web.spec.js` は未変更である。
- `lib.rs` の LOC が pass 2 完了時より減っている。

## 検証

第3パス実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

この pass の web 検証系には `cargo insta` ベースの snapshot suite は含まれていないため、snapshot 確認は追加しない。UI 回帰は既存 Playwright を正本とする。
また、この pass の spec では CI 相当の repo-wide aggregate check（例: `./scripts/check-all.sh`）は **スコープ外** とする。理由は、今回の変更対象が `apps/web` の局所的な code motion であり、pass 1 / pass 2 と同様に wasm cargo check + Playwright + trunk build + diff check を正本の受け入れ条件とするためである。

追加で、render symbol move を機械的に確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'fn circuit_content_height|fn draw_circuit|fn draw_palette|fn state_panel_layout|fn clamp_state_panel_offset|fn state_instances_for|fn draw_state_vector|struct StatePanelLayout|struct StateInstanceKey|struct StateInstanceCache' apps/web/src/lib.rs apps/web/src/render.rs
```

期待値:

- 列挙した render シンボルは `render.rs` 側にある
- `lib.rs` にはそれらの定義が残っていない
- method の場合は `impl QniApp` 内にあるためインデント有無に依存しない grep として扱う

追加で、`update` と wasm export が `lib.rs` 側に残っていることを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'impl eframe::App for QniApp|pub async fn start|pub async fn read_state_vector' apps/web/src/lib.rs apps/web/src/render.rs
```

期待値:

- `impl eframe::App for QniApp` は `lib.rs` のみにある
- `pub async fn start` と `pub async fn read_state_vector` は `lib.rs` のみにある

また、テストファイルが変更されていないことも確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/web/tests/web.spec.js
```

期待値:

- no output

LOC 確認として以下も実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/web/src/lib.rs apps/web/src/render.rs apps/web/src/gpu.rs apps/web/src/layout.rs apps/web/src/icons.rs
```

## 第4パスの判断基準

この pass 3 の完了後、残る大きな責務は主に次の 2 つに寄る想定である。

- `QniApp` の状態管理 / input / drag / update loop
- 色定義や gate/domain helper を含む app 側補助ロジック

したがって、pass 4 候補は次のルールで決める。

- app state / update loop が最大なら → `app.rs`
- domain/helper の塊が最大なら → その責務に応じた別モジュール

現時点の見立てでは、pass 3 後は **`app.rs` が第一候補** になる可能性が高い。ただし最終判断は、pass 3 完了後の実際の LOC と責務の塊を見て行う。
