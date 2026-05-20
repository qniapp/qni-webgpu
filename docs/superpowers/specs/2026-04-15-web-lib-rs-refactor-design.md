# web `lib.rs` 分割設計（第1パス）

## 背景
- `apps/web/src/lib.rs` は 2026-04-15 時点で 2561 LOC あり、責務が集中している。
- 現在の `lib.rs` には少なくとも以下が混在している。
  - app state / drag state
  - layout 計算と snap 判定
  - 回路・パレット描画
  - gate icon / SVG helper
  - WebGPU shader / resource / readback
- 今後の変更を安全に進めるため、次の機能追加より前に低リスクな分割を先に進める。
- ユーザー要望として、最終的には「1ファイルだいたい 500 LOC 以下」に近づけたい。ただし第1パスでは自然な境界を優先し、一時的な超過は許容する。

## 目的
- `lib.rs` から明らかに切り出せる責務を、安全に別モジュールへ分離する。
- 第1パスでは挙動変更なしの抽出に徹し、第2パス以降でより適切な構成へ寄せる。
- 分割後に、次に切るべき大きな責務（GPU / render / app）を判断しやすい状態を作る。

## 非目的
- 第1パスで `QniApp` の責務を全面再設計すること。
- 第1パスで `gpu.rs` 抽出まで完了すること。
- まとめて命名変更・API 変更・挙動変更を行うこと。
- すべてのファイルをこのパスだけで 500 LOC 以下に収めること。

## 採用方針
第1パスでは、依存が比較的薄く、抽出しても挙動に影響しにくい2領域を先に分離する。

1. `icons.rs`
2. `layout.rs`

`QniApp` 本体、update loop、GPU、state panel の大きな流れはまだ `lib.rs` に残し、まずは「安全に縮める」ことを優先する。

## 比較した案

### 案A: `icons.rs` / `layout.rs` をフラットに追加（採用）
- `apps/web/src/lib.rs` 直下に `mod icons; mod layout;` を追加する。
- 利点:
  - 変更範囲が最小。
  - import 経路が単純。
  - 第1パスとして安全。
- 欠点:
  - 第2パスで `gpu.rs` や `render.rs` を足すときに、さらに構成見直しが必要になる可能性がある。

### 案B: `ui/icons.rs` + `layout.rs`
- 利点:
  - 将来的に描画系を `ui/` 配下へ整理しやすい。
- 欠点:
  - 第1パスの段階では構成の先回りがやや強い。

### 案C: `render/icons.rs` + `math/layout.rs`
- 利点:
  - 概念上は整理しやすい。
- 欠点:
  - 現状では分割粒度が細かく、最小リスク方針に対して過剰。

## 第1パスのモジュール境界

### `apps/web/src/layout.rs`
ここには、主にレイアウト/スナップ計算を移す。
第1パスでは純化を目的にせず、`nearest_available_slot` のような `PlacedGate` 依存を含む軽い state 依存はそのまま許容する。

対象:
- `LayoutMetrics`
- `layout_metrics`
- `nearest_slot_center`
- `nearest_slot_index`
- `nearest_available_slot`
- `nearest_line`

意図:
- 座標計算、slot 探索、snap 判定を純粋な layout helper として分離する。
- `QniApp` 本体から「計算の詳細」を外して見通しを良くする。

### `apps/web/src/icons.rs`
ここには gate の見た目を作る drawing helper を移す。

対象:
- `SvgPoint`
- `map_svg_point_in_rect`
- `push_cubic_points_viewbox`
- `draw_gate_body`
- `draw_gate_body_fast`
- `draw_gate_icon`
- `draw_r_letter`
- `draw_s_curve`

意図:
- gate body / icon の描画責務を `QniApp` から切り離す。
- 将来的に `render.rs` を切る場合の下地にする。

### この段階で `lib.rs` に残すもの
- `QniApp`
- input / drag state 管理
- `draw_circuit`, `draw_palette`
- state panel 描画
- GPU / shader / readback
- 色定義 (`Colors`)
- gate/state のドメイン型 (`GateKind`, `PlacedGate`, `DragState` など)

意図:
- 第1パスは「責務の再設計」ではなく「安全な抽出」に限定する。
- 依存の強い塊は第2パス以降で切る。

## 実装ガードレール
第1パスは挙動変更なしの抽出に限定し、以下を原則とする。

- 移動対象の関数本体は、整形や import 解決を除いて原則そのまま移す。
- 関数シグネチャは、module 境界の都合で必要な可視性変更 (`pub(crate)`) を除き、原則変更しない。
- 定数値、描画パラメータ、しきい値、座標計算式は変更しない。
- 呼び出し順序や制御フローは変えない。
- 「ついで」の命名変更、ロジック整理、最適化は第1パスでは行わない。

## 実装手順
1. `layout.rs` を追加し、`lib.rs` 側に `mod layout;` を先に宣言する。
2. `lib.rs` から layout / snap helper を移す。
3. import と可視性 (`pub(crate)`) を最小限調整する。
4. `cargo check` を一度実行し、layout 抽出の失敗点を早期に切り分ける。
5. `icons.rs` を追加し、`lib.rs` 側に `mod icons;` を先に宣言する。
6. `lib.rs` から gate icon / SVG helper / body drawing を移す。
7. import と可視性 (`pub(crate)`) を最小限調整する。
8. `cargo check` を再度実行し、icons 抽出後の壊れ方を切り分ける。
9. 既存挙動が変わらないことを検証する。
10. 分割後の `lib.rs` を観察し、第2パス候補を決める。

## 受け入れ条件
- `lib.rs` から、仕様で列挙した `layout.rs` 対象シンボルと `icons.rs` 対象シンボルがすべて移動している。
- `lib.rs` には移動対象シンボルの定義が残っていない。
- 列挙した対象以外は原則このパスで新規抽出しない。
- `lib.rs` の LOC が分割前より減っている。
- `layout.rs` と `icons.rs` は、責務が説明可能な単位（layout 計算 / gate 描画 helper）として独立している。
- 第1パスで挙動差分を入れない。
- 既存のスナップ挙動と主要ゲートアイコン描画が維持されている。
- 第2パスの判断材料（GPU / render / app のどれが次に最大か）が見える。

## 検証
第1パス実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

追加の Rust 側テストや lint が存在する/追加した場合は、それも pass を確認する。
また、今回のリファクタで壊れやすい観点として以下を明示確認する。
- ドラッグ時のスナップ挙動
- 主要ゲートアイコン表示（特に palette / dragged gate）
必要なら既存 Playwright の関連ケースや手動確認を使って確認する。
必要なら reviewer を再度入れ、分割が不自然になっていないか確認する。

## 第2パスの判断基準
第1パス完了後、残った `lib.rs` の大きな塊を見て次を決める。

判断の主指標は **残っている責務ごとの LOC の大きさ** とし、同程度なら **依存の自然さ** を優先する。

- **GPU/shader/readback** が最大なら → `gpu.rs`
- **描画メソッド (`draw_circuit`, `draw_palette`, state panel)** が最大なら → `render.rs`
- **状態管理と update loop** が最大なら → `app.rs` 系の再配置

第2パス以降では、自然な境界を保ちつつ「各ファイルだいたい 500 LOC 以下」により強く寄せていく。
