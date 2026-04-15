# egui-web `gpu.rs` 分割設計（第2パス）

## 背景
- `apps/egui-web/src/lib.rs` は pass 1（`layout.rs` / `icons.rs` 抽出）後でも 2151 LOC あり、依然として大きい。
- pass 1 の最終レビューでは、残存責務の中で最もまとまりがあり、次に切り出す自然な候補として **GPU / shader / readback** が推奨された。
- 現在の `lib.rs` には、UI/drag/app state と並んで以下の GPU 責務がまとまって残っている。
  - state vector 用 compute shader / render shader
  - `StateVectorResources` による pipeline / buffer 管理
  - `StateVectorCallback` による egui-wgpu callback 実装
  - `GPU_READBACK` / `GpuReadbackState` / `read_state_vector` による readback 実装
- ユーザーはこの pass 2 について、**A: 純粋抽出のみ** を選択した。つまり、挙動変更や積極的な再編は行わず、既存の GPU 塊を安全に `gpu.rs` へ移すことを優先する。

## 目的
- `lib.rs` から GPU / shader / readback の責務を `apps/egui-web/src/gpu.rs` に安全に分離する。
- 挙動変更なしの code motion に徹し、将来の `render.rs` / `app.rs` 分割に向けて `lib.rs` の責務境界をさらに見やすくする。
- wasm export と既存 Playwright / trunk / WebGPU 動作確認フローを壊さない。

## 非目的
- shader ロジックや buffer 初期化順の変更。
- `QniApp` や input / drag / palette / circuit 描画の再設計。
- `GateParams` / `gate_params` / `gate_params_controlled` / `gate_matrix` の責務移動。
- `StateInstanceCache` の再配置。
- `read_state_vector` の公開 API 変更。
- この pass だけで全ファイルを 500 LOC 以下にすること。

## 採用方針
第2パスでは **GPU 実装に閉じた責務のみを `gpu.rs` に移す**。ユーザー選択どおり、純粋抽出のみを採用し、次のような「ついで変更」は行わない。

- visibility の積極的整理
- helper 命名変更
- shader 文字列の分割や外部ファイル化
- callback / readback 制御フローの整理
- 業務ロジック（gate 列→GPU 入力生成）の GPU モジュールへの吸い込み

これにより、今回の pass は「アプリ側が GPU 用入力を作り、GPU モジュールが描画・readback を担当する」という責務線を作ることに限定する。

## 比較した案

### 案A: 純粋抽出のみ（採用）
- `gpu.rs` を追加し、shader / resources / callback / readback をそのまま移す。
- `lib.rs` には `QniApp`、state panel、gate パラメータ生成、UI 入力処理を残す。
- `#[wasm_bindgen] pub async fn read_state_vector()` は `lib.rs` に薄い wrapper を残し、実処理を `gpu.rs` に委譲する。
- 利点:
  - 変更リスクが最小。
  - pass 1 と同じ「挙動変更なし」の進め方を維持できる。
  - 既存の wasm export 面を大きく動かさずに済む。
- 欠点:
  - `gpu.rs` は一部 `crate::Colors` や `crate::GateParams` に依存したままになる。
  - きれいな最終形ではなく、中間段階としての抽出になる。

### 案B: 純粋抽出 + 軽い境界整理
- 案Aに加えて visibility や helper 配置を少し整える。
- 利点:
  - モジュール境界がやや明瞭になる。
- 欠点:
  - code motion だけではなくなり、レビュー時の差分判断が難しくなる。
  - A に比べて挙動変更混入リスクが上がる。

### 案C: shader / GPU 補助型まで積極再編
- shader 定数の分離や `GateParams` / `StateInstanceCache` まで見直す。
- 利点:
  - 将来の最終形に近づけやすい。
- 欠点:
  - 今回のユーザー選択（A）と合わない。
  - 変更面積が大きく、失敗時の切り分けが難しい。

## 第2パスのモジュール境界

### 新規追加: `apps/egui-web/src/gpu.rs`
ここには、**GPU 実装そのものに閉じた責務**を移す。

対象:
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
- `read_state_vector` の内部実装本体（wrapper ではない実処理）

意図:
- shader / pipeline / callback / readback を 1 モジュールに閉じ込める。
- `lib.rs` から WebGPU 詳細を追い出し、アプリの流れを読みやすくする。

### `apps/egui-web/src/lib.rs` に残すもの
対象:
- `QniApp`
- `StatePanelLayout`
- `StateInstanceCache`
- `Colors`
- `GateParams`
- `gate_params`
- `gate_params_controlled`
- `gate_matrix`
- `collect_gate_params`
- state panel / circuit / palette 描画
- wasm export の公開面 (`start`, `read_state_vector` wrapper)

意図:
- `lib.rs` には「アプリが何を描き、どんな入力を GPU に渡すか」を残す。
- GPU モジュールに、量子回路の業務ロジックまで背負わせない。

## 依存関係の方針
この pass で目指す主方向は **`lib.rs` → `gpu.rs`** である。ただし純粋抽出を成立させるため、pass 2 では以下の **例外的な親参照だけを明示許可** する。

許可する `gpu.rs` → `crate::...` 参照:
- `crate::Colors`
- `crate::GateParams`

想定される依存:
- `gpu.rs` → `crate::Colors`
- `gpu.rs` → `crate::GateParams`
- `lib.rs` → `gpu::StateInstance`
- `lib.rs` → `gpu::RenderColors`
- `lib.rs` → `gpu::StateVectorCallback`
- `lib.rs` → `gpu::read_state_vector_impl(...)` のような内部 helper

上記以外の `gpu.rs` → `crate::...` 参照追加は、この pass では行わない。
この pass では依存を完全にきれいにすることよりも、**責務の大きな塊を安全に外へ出すこと**を優先する。

## `read_state_vector` の扱い
`read_state_vector` は wasm から呼ばれる公開エントリポイントなので、公開面を不必要に動かさない。

採用方針:
- `#[wasm_bindgen] pub async fn read_state_vector()` は `lib.rs` に残す。
- 実際の readback 本体は `gpu.rs` に移し、`lib.rs` の export 関数は薄い委譲 wrapper にする。

意図:
- wasm export の位置変更による余計なリスクを避ける。
- 一方で readback の実装詳細は `gpu.rs` に閉じる。

## 実装ガードレール
この pass は挙動変更なしの抽出に限定し、以下を原則とする。

- 移動対象の関数/impl/定数本体は、import 解決と最小限の可視性調整を除いて原則そのまま移す。
- 可視性変更は **必要最小限** に限定し、外部公開 (`pub`) は追加しない。必要なら `pub(crate)` までとする。
- shader 文字列の内容は変更しない。
- buffer / bind group / pipeline の生成順は変更しない。
- callback の `prepare` / `paint` の制御フローは変更しない。
- `GPU_READBACK` の更新タイミングは変更しない。
- `read_state_vector` の戻り値・エラー文言・非同期の流れは変更しない。
- `start` を含む wasm export の関数名・シグネチャ・公開位置は維持する。
- `GateParams` や app 側 state 生成ロジックは移さない。
- 命名変更、最適化、整理、コメント追加などの「ついで変更」はしない。

## 実装手順
1. `apps/egui-web/src/gpu.rs` を追加し、`lib.rs` に `mod gpu;` を宣言する。
2. `StateInstance`, `RenderParams`, `RenderColors`, shader 定数を `gpu.rs` に移す。
3. `StateVectorResources` とその `impl` を `gpu.rs` に移す。
4. `StateVectorCallback` と `egui_wgpu::CallbackTrait` 実装を `gpu.rs` に移す。
5. `GpuReadbackState` と `GPU_READBACK` を `gpu.rs` に移す。
6. `read_state_vector` の実処理本体を `gpu.rs` へ移し、`lib.rs` 側には薄い wrapper を残す。
7. `lib.rs` 側の import を最小限調整し、`StateInstanceCache` などが `gpu::StateInstance` を参照するようにする。
8. `cargo check --target wasm32-unknown-unknown` を早い段階で実行して import / visibility 崩れを切り分ける。
9. 既存 Playwright / trunk build / diff check を再実行し、挙動差分がないことを確認する。
10. 最後に symbol move と LOC を確認し、pass 3 候補を見直す。

## 受け入れ条件
- `apps/egui-web/src/gpu.rs` が追加されている。
- `lib.rs` から以下の定義が `gpu.rs` へ移っている。
  - `StateInstance`
  - `RenderParams`
  - `RenderColors`
  - `STATE_WORKGROUP_SIZE`
  - `STATE_COMPUTE_SHADER`
  - `STATE_RENDER_SHADER`
  - `StateVectorResources`
  - `StateVectorCallback`
  - `GpuReadbackState`
- `GPU_READBACK` は `gpu.rs` 側にある。
- `read_state_vector` の内部実装本体は `gpu.rs` にあり、`lib.rs` 側には薄い委譲 wrapper だけが残っている。
- `start` と `read_state_vector` の wasm export 名・シグネチャ・公開位置は互換を維持している。
- `QniApp` / drag / palette / circuit / state panel / gate parameter generation は `lib.rs` に残っている。
- `lib.rs` の LOC が pass 1 完了時より減っている。
- 既存の Playwright / build / wasm cargo check が通る。
- pass 3 候補（`render.rs` or `app.rs`）を判断しやすくなっている。

## 検証
第2パス実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

この pass の egui-web 検証系には `cargo insta` ベースの snapshot suite は含まれていないため、snapshot 確認は追加しない。UI 回帰は既存 Playwright を正本とする。

追加で、symbol move を機械的に確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n '^(pub\(crate\) )?(struct StateInstance|struct RenderParams|struct RenderColors|const STATE_WORKGROUP_SIZE|const STATE_COMPUTE_SHADER|const STATE_RENDER_SHADER|struct StateVectorResources|struct StateVectorCallback|struct GpuReadbackState)' apps/egui-web/src/lib.rs apps/egui-web/src/gpu.rs
```

期待値:
- 列挙した GPU シンボルは `gpu.rs` のみにある
- `lib.rs` にはそれらの定義が残っていない

また、LOC 確認として以下も実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/egui-web/src/lib.rs apps/egui-web/src/gpu.rs apps/egui-web/src/layout.rs apps/egui-web/src/icons.rs
```

## 第3パスの判断基準
この pass 2 の完了後、残る大きな責務は主に次の2つに寄る想定である。

- circuit / palette / state panel の描画
- `QniApp` の状態管理 / update loop / input handling

したがって、pass 3 候補は次のルールで決める。

- 描画責務が最大なら → `render.rs`
- app state / update loop が最大なら → `app.rs`

現時点の見立てでは、pass 2 後は **`render.rs` が第一候補** になる可能性が高い。ただし最終判断は、pass 2 完了後の実際の LOC と責務の塊を見て行う。