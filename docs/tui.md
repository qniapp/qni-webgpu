# TUI PoC

この PoC は、Web 版と独立してターミナルに可変量子ビット数の回路と状態ベクトルを表示する。
Rust + ratatui で最小のリッチ表示を行う。

## 実行方法

```
cd apps/tui
cargo run
```

補足: `snapshot_dump` を実行する場合は `cargo run --bin snapshot_dump` を使う。

## 出力例

```
[H] [CTRL] [X] [Y] [Z] [√X] [S] [S†] [T] [T†] [SWAP]
──────────────

q0: ---------
q1: ---------

(...)

|00> 1.000 ########
|01> 0.000 
|10> 0.000 
|11> 0.000 
```

## 操作

- 起動時は回路が空で、状態ベクトルは |0> を示す。
- パレット上のゲートをマウスで回路線へドラッグ&ドロップすると配置できる。
- 配置済みゲートを回路の外にドラッグ&ドロップすると削除される。
- ドラッグ中はゲートがマウス位置に追従して表示される。
- ドラッグ中のゲートはシアン色で表示される。
- ゲートは影付きの箱アイコンとして表示される。
- ゲートをつかむと最下段に空の量子ビットが追加される。
- 空の列ができた場合は、ゲートが左に詰められる。
- パレットと回路の間に区切り線が表示される。
- ドラッグ中のスロットプレースホルダ表示はしない。
- 既にゲートがあるスロットにはスナップしない。
- 隣り合うゲートの間にドロップすると列を挿入し、右側のゲートが1列右へ移動する。
- 追加された最下段の量子ビットが空のままならドロップ時に削除される。
- q0 と q1 は常に保持される。
- 量子ビット数は最大 16 まで。
- 状態ベクトルは表示領域の最下段に円で表示される（量子ビット数に応じて 2^n 個）。表示順は |00>, |01>, |10>, |11> ... の順（q0 が最下位ビット）。
- 円の塗りつぶし量で確率を示し、円周上の線で位相を示す（位相 0 は 12 時方向）。
- 振幅 0 の状態は灰色の円で表示し、位相線は描画しない。
- 表示領域が足りない場合は収まる範囲だけ表示する。
- 円の上にホバーすると、振幅・確率・位相のポップアップが表示される。
- ポップアップは状態ベクトル領域の上部に表示され、円とは重ならない。
- パレットには `H, X, Y, Z, √X, S, S†, T, T†, Φ, Rx, Ry, Rz, SWAP, CTRL, MEASURE` を表示する。
- 画面幅が不足する場合、パレットは複数行に折り返して表示する。
- ホバー中の列の右端に未確定ライン（灰色）を表示する。
- 最左ゲートより左にホバーした場合も、未確定ラインを表示する。
- クリックすると確定ライン（青）として固定し、そのラインまでの状態ベクトルを表示する。
- ホバー中は未確定ライン位置に合わせて状態ベクトルが更新される。
- 確定ラインは次の確定が行われるまで維持される。
- SWAP は太字の `X` 一文字で描画する。
- 同じ列に SWAP が2つある場合、その2量子ビットを入れ替える。
- 同じ列に SWAP が2つある場合、間を縦線で接続して表示する。
- CTRL は小さな四角として描画し、ラベルは表示しない。
- CTRL はドラッグ中のみ枠線付きで表示する。
- 同じ列に CTRL と X がある場合、CNOT として動作する。
- CTRL と X が同じ列にある場合、CTRL/X をすべて縦線で接続して表示する。
- 同じ列に複数の CTRL と X がある場合、複数制御（CCX など）として動作する。
- CNOT/SWAP の接続線は、関係のないゲートが同じ列にある場合は上下に少し空白を入れて途切れて見えるようにする。
- MEASURE はパレット上では `M` として描画し、回路上では測定結果の `0` または `1` を表示する。
- MEASURE は測定を行い、状態ベクトルを測定結果に従ってコラプスする（確率に応じて 0/1 をランダムに選ぶ）。
- Φ は位相ゲート（φ = π/2）として扱い、回路上では上段に `π/2` を表示する。
- Φ の値をクリックするとインライン編集できる（`π`, `3π/4`, `2π` など）。不正な値はエラー表示になる。
- Rx/Ry/Rz も同じ形式で角度を編集できる（デフォルトは `π/2`）。
- 角度編集中はラベル末尾にカーソルを表示する。
- 未確定・確定ラインがない場合は、開始位置（|0>）に確定ラインを表示する。
- `q` を押すと終了確認モーダルが表示され、Yes を選ぶと終了する。

## テスト

```
cd apps/tui
cargo test
```

### スナップショット（insta）

```
cd apps/tui
INSTA_UPDATE=always cargo test --test snapshots
```

### スナップショットの目視（PNG化）

文字のみの PNG を出力する。

```
cd apps/tui
python3 scripts/render_snapshot_png.py \
  tests/snapshots/snapshots__snapshot_two_qubit_state_circles.snap \
  /tmp/tui-two-qubit.png
```

カラーの PNG を出力する。

```
cd apps/tui
cargo run --bin snapshot_dump -- --out tests/snapshots/snapshot_two_qubit_state_circles.dump
python3 scripts/render_snapshot_png.py \
  tests/snapshots/snapshot_two_qubit_state_circles.dump \
  /tmp/tui-two-qubit-color.png
```

### ターミナル描画のPNG化（terminal-screenshot）

xterm.js で ANSI をレンダリングして PNG 化する。人間側の端末とフォントを揃えると見た目のズレが小さくなる。
（初回は `npx` 経由で `terminal-screenshot` を取得するためネットワークが必要）

```
cd apps/tui
./scripts/tui_terminal_screenshot.sh
```

フォントや出力先を指定する場合:

```
TUI_FONT_FAMILY="Caskaydia Mono Nerd Font" \
TUI_SCREENSHOT_OUT=/tmp/tui-terminal-screenshot.png \
./scripts/tui_terminal_screenshot.sh
```

## E2E（ratatui-testlib）

PTY 経由で TUI を起動し、マウス入力を注入して画面状態を検証する。

```
cd apps/tui
cargo test --test e2e
```

補足:
- `ratatui-testlib` は `apps/tui/vendor/ratatui-testlib` にローカルパッチを当てている

## 描画の実装メモ（現行）

TUI のゲート描画は `apps/tui/src/render.rs` の `draw_gate_box` に集約されている。

- 共通: ゲート色（background）を基準にし、上下にハイライト/シャドウの線を描く。
  - ハイライト: `▔`（fg=highlight / bg=background）
  - シャドウ: `▁`（fg=shadow / bg=background）
  - 内側は ` `（スペース）を `fg=text / bg=background` で塗る。
- X / Φ: 他ゲートと同じ四角形の描画（8角形の角表現は使わない）。
- Φ / Rx / Ry / Rz: 位相ラベルはゲート矩形の 1 行上に表示。
  - 編集中は反転表示（`fg=UI_BACKGROUND / bg=background`）

## 文字選定メモ（TUI での安全性）

端末描画ではフォント差・レンダラ差が出やすいため、文字選定に注意が必要。

- もっとも安全: ASCII（U+0020–U+007E）。
- 比較的安全: Box Drawing（U+2500–U+257F）と Block Elements（U+2580–U+259F）。
  - 罫線/ブロックは端末 UI を想定した字形で、ズレが出にくい。
  - 参考: Unicode ブロック一覧
    - Box Drawing: https://unicode.link/blocks/box-drawing
    - Block Elements: https://unicode.link/blocks/block-elements
  - 参考: Unicode Core Spec（Chapter 22）
    - https://unicode.org/versions/Unicode17.0.0/core-spec/chapter-22/
- 要注意: 幾何記号（特に三角 `◢◣◥◤` など）。
  - フォントにより字形がセル全面を埋めないため、上下左右に隙間/ズレが出る。
- PUA / Nerd Fonts 系の記号は環境依存が大きいため、使用時は実機フォントで確認する。
  - 参考: Nerd Fonts（フォント依存）
    - https://www.nerdfonts.com/
  - 参考: DEC Special Graphics（端末の罫線セット）
    - https://en.wikipedia.org/wiki/DEC_Special_Graphics
