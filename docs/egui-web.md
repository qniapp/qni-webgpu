# egui WebGPU PoC (Rust)

## Prerequisites
- `rustup target add wasm32-unknown-unknown`
- `cargo install trunk`

## Run (local)
まずサーバを起動する。
```
cd apps/egui-web
trunk serve --address 127.0.0.1 --port 4174 --no-autoreload
```
Open: `http://127.0.0.1:4174/`

ローカル開発では通常の Chrome で上記 URL を開く。WebGPU 用の特別な起動フラグは不要。
リポジトリルートから helper script を使う場合も、通常起動の Chrome を開くだけにする。
```
./scripts/open-egui-web.sh
```
この script は `google-chrome-stable` を最優先で探し、見つからない場合のみ Chromium 系へ fallback する。

明示的にブラウザを固定したい場合の例:
```
QNI_EGUI_WEB_BROWSER=/usr/bin/google-chrome-stable ./scripts/open-egui-web.sh
```

環境変数:
- `QNI_EGUI_WEB_BROWSER`: 使用する Chromium 系ブラウザを明示
- `QNI_EGUI_WEB_PORT`: 接続先ポートを変更
- `QNI_EGUI_WEB_URL`: 接続先 URL を直接指定

## Agent browser / visual workflow
アプリ内ブラウザは `http://127.0.0.1:4174/` を開ける。
WebGPU の実描画確認では、通常の外部 Chrome か Playwright 経路を使う。

基本の手順:
```
cd apps/egui-web
trunk serve --address 127.0.0.1 --port 4174 --no-autoreload
```

別ターミナルで外部 Chrome を開く:
```
cd /home/yasuhito/Work/qni-webgpu
./scripts/open-egui-web.sh
```

エージェントからスクリーンショットや操作込みで確認する場合は、既存サーバを使って headed Playwright を走らせる。
この経路は CI/headless 安定化用に `test-support/browser-launch.ts` の Playwright 起動設定を使う。
```
cd apps/egui-web
QNI_EGUI_WEB_EXTERNAL_SERVER=1 HEADLESS=0 pnpm exec playwright test --grep 'egui webgpu canvas renders content' --workers=1
```

エージェントからゲートを意味ベースでドラッグアンドドロップする場合は、専用 CLI を使う。
この CLI は Playwright で Chrome を起動し、`H:q0:0` のような指定を palette gate → circuit slot のドラッグへ変換する。
目視確認用にページ全体 screenshot を保存し、`window.__eguiReadStateVector()` も JSON で出力する。
```
cd apps/egui-web
QNI_EGUI_WEB_EXTERNAL_SERVER=1 node -r ts-node/register/transpile-only scripts/agent-visual.ts drag \
  --gate H --wire q0 --slot 0 \
  --out output/playwright/agent-visual/h-q0.png

QNI_EGUI_WEB_EXTERNAL_SERVER=1 node -r ts-node/register/transpile-only scripts/agent-visual.ts ops \
  --ops H:q0:0,C:q0:1,X:q1:1 \
  --out output/playwright/agent-visual/bell.png
```
Anti-control は `anti-control:q0:0` / `anti:q0:0` / `◦:q0:0` で指定できる。
|0⟩ / |1⟩ は `|0>:q0:0` / `write0:q0:0` / `|1>:q0:0` / `write1:q0:0` などで指定できる。

`scripts/agent-visual.ts` は通常の `@playwright/test` 用 SwiftShader launch ではなく、screenshot が黒くならない agent visual launch を使う。
現状の egui content margin に合わせて drop 座標に `--vertical-offset 8` を既定で加える。
UI の外枠や egui panel margin を変えた場合は、この値を一時的に上書きして確認する。
```
QNI_EGUI_WEB_EXTERNAL_SERVER=1 node -r ts-node/register/transpile-only scripts/agent-visual.ts drag \
  --gate X --wire q1 --slot 2 \
  --vertical-offset 8
```

手動デバッグで DevTools/CDP 接続が必要なときだけ、専用 profile と remote debugging port を使って Chrome を起動する。
WebGPU 用の特別なフラグは付けない。
```
/usr/bin/google-chrome-stable \
  --user-data-dir=/tmp/qni-webgpu-chrome-agent-verify \
  --new-window \
  --no-first-run \
  --no-default-browser-check \
  --remote-debugging-port=9222 \
  http://127.0.0.1:4174/
```

確認すること:
- `navigator.gpu` が `true`
- `[data-testid="webgpu-error"]` が非表示
- `await window.__eguiReadStateVector()` が非空の配列を返す
- ページスクリーンショットでパレット、q0/q1 のライン、状態ベクトル表示が見える
- UI 操作を伴う変更では Playwright の `egui webgpu canvas renders content` など、該当する描画テストを通す

スクリーンショットや一時アーティファクトは `apps/egui-web/output/playwright/` か Playwright の `test-results/` に置き、必要な確認が終わったらコミット対象にしない。
OS 全体のスクリーンショットが取れない環境では、Playwright の `page.screenshot()` / `locator('#egui-canvas').screenshot()` を使う。

## Playwright / Cucumber rollout
```
cd apps/egui-web
pnpm install
pnpm exec playwright install chromium
pnpm run test:preflight
pnpm run test:bdd
pnpm run test:pw-legacy
```
初回導入では Cucumber の Markdown Gherkin (`.feature.md`) を staged rollout で追加している。
`features/**/*.feature.md` は `@cucumber/cucumber` で実行し、`pnpm run test:bdd` がその入口になる。
一方で既存の `@playwright/test` suite はまだ正本として残しており、`pnpm run test:pw-legacy` と `pnpm test` はどちらも `playwright test` を実行する。
つまり **初回 pass では `test` を BDD へ切り替えない**。

Step definitions は TypeScript へ移行済み。
`cucumber.ts` は Cucumber v12 の `default` profile として定義し、`ts-node/register` を読み込む。
対象は `features/step_definitions/**/*.ts` のみにする。
新規 step は `.steps.ts` で書き、`pnpm run typecheck` と `pnpm run test:bdd` の両方で確認する。
現在の 3 本の代表 scenario の step definitions は `.steps.ts` に移行済みで、共有する support 型は `features/support/support-types.ts` に置く。
TS 化済み browser bootstrap / support module / CLI / config / suite / Node preflight tests は `bootstrap.ts`、`cucumber.ts`、`features/support/bootstrap.ts`、`features/support/assertions.ts`、`features/support/browser.ts`、`features/support/egui-helpers.ts`、`features/support/hooks.ts`、`features/support/server.ts`、`features/support/world.ts`、`playwright.config.ts`、`scripts/agent-visual.ts`、`test-support/agent-visual-command.ts`、`test-support/browser-launch.ts`、`test-support/web-server.ts`、`tests/egui-web.spec.ts`、`test-node/*.test.ts`。
`bootstrap.ts` は Trunk の `pre_build` hook で `.trunk-generated/bootstrap.js` へ emit し、`index.html` はその生成物を `bootstrap.js` として copy する。

BDD 化したのは最初の 3 scenario のみ:
- `startup-success.feature.md`
- `plain-chromium-error.feature.md`
- `drag-preview-z-order.feature.md`

MCP から Playwright を使う場合は、helper の `scripts/playwright-mcp.sh` を使うと
`.playwright-mcp/config.json` を自動検出しつつ `--isolated` 付きで起動できる。
（このプロジェクトの `.mcp.json` でも同等の設定を直接記述している。）
WebGPU は X がないと adapter が取れないため、Xvfb を挟んで実行する。
```
cd apps/egui-web
xvfb-run -a -s "-screen 0 1920x1080x24" pnpm run test:pw-legacy
```
`@playwright/test` の headless shell だと SRI mismatch が起きるため、
`playwright.config.ts` は Playwright 同梱ブラウザよりも先に `google-chrome-stable` を優先し、
見つからない場合のみ `chromium.executablePath()` に fallback する。
必要なら `PLAYWRIGHT_CHROMIUM_PATH` で明示上書きできる。
この browser policy と `trunk serve --address 127.0.0.1 --port 4174 --no-autoreload` の server policy は
`test-support/browser-launch.ts` と `test-support/web-server.ts` に集約されており、
legacy Playwright と BDD の両方が同じ shared source of truth を使う。
後方互換用の `playwright-browser.cjs` wrapper は残さず、呼び出し側は直接 `test-support/browser-launch.ts` を参照する。
ローカルの手動起動は通常 Chrome、Playwright 経路は CI/headless 安定化用の起動設定を使う。

repo root の `scripts/check-all.sh` でも staged rollout を維持し、
`test:preflight` → `test:bdd` → `test:pw-legacy` の順で Web の gate を通す。

## Performance

CLAUDE.md の方針 (「WebGPU の恩恵を最大限に得る」「production で CPU readback しない」) に対する現状を以下に記録する。詳細な audit 結果は `docs/egui-web-perf-audit.html`。

### Production パスの GPU 常駐性

- 状態ベクトル / Bloch / 測定の値はすべて GPU storage buffer に置き、render shader が直接 sample する (`STATE_RENDER_SHADER` パターン)。CPU リードバックなし。
- `read_state_vector_impl` / `read_bloch_vectors_impl` / `read_measurement_outcomes_impl` は `#[wasm_bindgen]` 経由 JS から呼ぶ test 専用。production の `prepare()` 経路は通らない (`apps/egui-web/src/gpu/readback.rs`)。

### recompute あたりの GPU 往復

| 項目 | 旧 | 現 |
|---|---:|---:|
| `queue.submit` 呼び出し / recompute | N (gate ごと) | **1** |
| `\|0…0⟩` 初期化のための CPU 確保 + upload | 2^N × 8 byte | **0** (encoder 内で `clear_buffer` + 8 byte `copy_buffer_to_buffer`) |
| アイドルフレームの params `queue.write_buffer` | 3 / frame | **0** (dirty flag) |

設計のキモ:
- 全 gate / Bloch capture / 測定 dispatch を **1 つの encoder** にまとめる。各 op の params は recompute 開始時に staging buffer へ一括 upload しておき、ループ内で `copy_buffer_to_buffer` から個別に取り出す (`gpu/callbacks.rs` の `StateVectorCallback::prepare`)。
- 「encoder を外に出すだけ」では `queue.write_buffer` が submit 前にまとめて実行される仕様で壊れるため、staging + intra-encoder copy を採用 (動的 uniform offset でも実現可能だが bind group layout 変更が増えるので未採用)。
- params 用の `Option<*Params>` を `StateVectorResources` に保持し、`prepare()` で前フレームと等しいときは `queue.write_buffer` を skip する。viewport / colors はほとんど変化しないので、idle frame では params 系は完全に無 upload。

### Debug HUD

`Backquote` (`) キーで右上に FPS / フレーム時間の overlay を出せる (off ↔ on トグル)。OFF の間は何のコストもなく、ON の間は連続再描画を強制するので perf 計測には注意。F12 は Chrome DevTools と競合するので避けた。

実装: `apps/egui-web/src/app/update_flow.rs` の update tail と `apps/egui-web/src/app/fps_hud.rs`。`ctx.input(|i| i.stable_dt)` を 60 frame の `VecDeque` に貯めて移動平均。

### 計測メモ

実 wall-clock で改善を検出したい場合は **30+ ゲートの recompute** を含むシナリオが必要。既存 Playwright suite (1〜5 gates / test) では submit 削減ぶん (~50–500 μs) が browser 起動 / drag 機構 / readback の 1,100 ms+ ノイズに埋もれる。

実測例 (10 runs each, `--repeat-each 10`):

| テスト | 改修前 中央値 | 改修後 中央値 |
|---|---:|---:|
| `applies a unitary chain` | 1,251 ms | 1,280 ms |
| `GPU bloch reduction` | 1,130 ms | 1,132 ms |

ノイズと同程度。**構造改善は事実だが、現行テストでは検出限界以下**。深い回路向けの「先行投資」と理解しておく。

## Notes
- `apps/egui-web/src/lib.rs` uses eframe with the `wgpu` feature enabled.
- 通常のブラウザ起動で利用可能な WebGPU adapter が見つからない場合、キャンバスが白いままになる代わりに、ページ上に WebGPU 初期化失敗メッセージを表示する。
- ローカル手動確認は通常の Chrome で行う。`./scripts/open-egui-web.sh` も WebGPU 用の特別な起動フラグは付けない。
- 状態ベクトルの計算と円描画は WebGPU（Compute/Fragment）で行い、CPU への読み戻しはテスト時のみ。
- `window.__eguiReadStateVector()` は非同期（Promise）で、Playwright は await して検証する。
- The Playwright test drags the H gate onto q0, waits for `window.__eguiReadStateVector()` to match the expected amplitudes, and checks that the canvas contains non-background pixels.
- The Playwright run writes screenshots to `/tmp/qni-egui-webgpu-initial.png` and `/tmp/qni-egui-webgpu-after.png`.
- Playwright uses `trunk serve --no-autoreload` to keep the canvas DOM stable during screenshots.
- State circles use shader-side AA (fwidth + smoothstep) for fill/outline/needle to reduce jagged edges.
- The circle quad now expands to include stroke width to avoid flat/clipped edges.
- The vertex quad adds a small pad (1px) so the AA fringe isn't clipped at the bounds.
- 全ゲート dispatch は 1 つの encoder にまとめて 1 回 `queue.submit` する。各ゲートの params は事前に staging buffer へ packing し、ループ内で `copy_buffer_to_buffer` で `gate_params_buffer` へ書き戻す。これで「最後の write_buffer が全 dispatch に効いてしまう」問題を避けつつ、recompute あたりの IPC 往復を N → 1 に圧縮している (gpu/callbacks.rs)。
- Control gates render as a qni-style standalone filled dot, not as a labeled rectangular button.
- Anti-control gates render as a qni-style standalone open circle and control on the zero state.
- |0⟩ / |1⟩ gates draw qni's bracket icon plus the literal digit, and follow qni's simulator semantics: per-pair conditional X (no-op when the qubit is in superposition, X when it sits in the opposite basis state).
- BlochDisplay は回路にゲートとして並ぶがユニタリではなく観測専用。`linearize_ops` が列ごとに GPU の Bloch capture を挿入し、`BLOCH_REDUCE_SHADER` がその時点の縮約密度行列由来の (x, y, z) を計算する（qni: `packages/simulator/src/state-vector.ts:blochVector`、`matrix.ts:qubitDensityMatrixToBlochVector` と同じ意味論）。x = 2·Re(ρ_01), y = -2·Im(ρ_01), z = ρ_00 - ρ_11。CPU ミラーシミュレータは使わない。
- BlochDisplay の見た目は qni の `bloch_display.css` に揃える: bg-green-50 (#f0fdf4)、軸 / 球境界 gray-400 (#9ca3af)、ベクトル線 gray-900 (#111827)、矢印先 red-500 (#ef4444)、ゼロベクトルのみ blue-500 (#3b82f6)。
- 投影は qni の DOM 変換 `rotateY(phi) rotateX(-theta)` + `perspective: 4rem` + `perspective-origin: top right` をそのまま再現（pinhole 投影、p = 4·radius、origin = (1, -1) in radius units）。Bloch → CSS 軸対応は +x → +z (奥行き、視点向き)、+y → +x (右)、+z → -y (上)。
- 結果として Bloch (1,0,0) (|+⟩, H|0⟩) は短く左下へ前縮小、Bloch (0,0,1) (|0⟩) は真上、(0,0,-1) (|1⟩) は真下、(0,±1,0) (|±i⟩) は真横、Bloch (-1,0,0) (|-⟩) は右上奥に縮小される。
- 球の装飾線は qni の SVG (横線・縦線・NE/SW 斜線、垂直細楕円 rx=18% ry=50%、水平細楕円 rx=50% ry=18%) を踏襲し、傾けない。
- ベクトル長 ≈ 0 のとき (もつれて部分トレースが maximally mixed になる、palette 表示、ドラッグ中、未スナップなど) は qni の `data-d='0'` ルール通り中心に blue-500 の点だけを描画し、線は引かない。Bell 状態の各量子ビットが好例。
- Measurement ゲートは qni `simulator.ts:measure` の意味論を GPU で実行する: 列ごとに `MEASURE_REDUCE_SHADER` が pZero を計算し、決定論的 RNG (gate id ベース) で 0/1 をサンプル。続く `MEASURE_COLLAPSE_SHADER` が選ばれた基底に状態を射影して √(p) で再正規化する。結果は GPU state buffer の ping-pong で次の列に持ち越されるため、State vector は collapse 後の状態を表示する。
- Measurement の見た目は qni `measurement_gate.css` に合わせる: パレット / 未測定時はメーターを intermediate purple (#a855f7)、回路に置いて結果が出ると `text-zinc-200` (#e4e4e7) のメーターに切り替え、`0` は red-500・`1` は blue-500 で中央にオーバーレイする。
- 状態ベクトル計算は GPU の `STATE_COMPUTE_SHADER` (per-gate dispatch + ping-pong) で実行する。CPU 側にミラーシミュレータは残っていない (Step 4 で削除済み)。`simulation_plan/*` は `linearize_ops` で配置済みゲートを GPU op ストリームに並び替え、capacity を検証するだけのオーケストレーション専用モジュール。AGENTS.md ルール「シミュレーションは GPU でのみ」を満たしている。
- Measurement は GPU 完結。`MEASURE_REDUCE_SHADER` (workgroup_size=64) で pZero を reduction し、決定論的 PCG を gate id でシードして r をサンプル、`(pZero, r, outcome, sqrt_p_kept)` を `measurement_aux_buffer[slot]` に書く。続けて `MEASURE_COLLAPSE_SHADER` が同じ aux スロットを読み、生き残った基底側の振幅を `1/sqrt_p_kept` で正規化、もう一方を 0 にして state buffer の ping-pong 反対側に書き込む。
- `read_measurement_outcomes()` (wasm_bindgen) で `[gate_id, outcome, …]` を取得できる。`window.__eguiReadMeasurementOutcomes()` 経由で Playwright から `readMeasurementOutcomes` ヘルパーが返す `MeasurementOutcome[]` を assert する。
- Bloch ベクトル計算は GPU の `BLOCH_REDUCE_SHADER` (workgroup_size=64 で 1 workgroup の reduction) が処理する。`linearize_ops` が列ごとに `ApplyGate` と `CaptureBloch` を交互に並べ、`gpu.rs::prepare` がそれを順番に dispatch する。出力は `bloch_output_buffer` に shader-side で書き込まれる。
- 描画は GPU 完結: `BLOCH_OVERLAY_SHADER` (vertex+fragment) が `bloch_output_buffer` を直接 sample してアロー線・先端ドットを描く (`BlochOverlayCallback`)。`MEASUREMENT_DIGIT_SHADER` も同様に `measurement_aux_buffer.z` (outcome) を sample して `0` を SDF 円、`1` を SDF 縦棒で描画する。本番経路で GPU → CPU リードバックは発生しない。
- `read_bloch_vectors()` / `read_measurement_outcomes()` (wasm_bindgen) は **テスト専用の async on-demand readback**: 呼ばれた時に staging buffer + map_async で `[gateId, x, y, z, …]` / `[gateId, outcome, …]` を返す。本番のレンダーには影響しない。Playwright は `readBlochVectors` / `readMeasurementOutcomes` ヘルパー経由で await して使う。
- Spacer ゲートは qni `packages/elements/icon/spacer-gate.svg` を踏襲した装飾専用 NOP。viewbox 上の (9,21)–(15,27)、(21,21)–(27,27)、(33,21)–(39,27) に塗りつぶし矩形 3 つで `…` を描く。色は `text-neutral-900` (#171717)。`simulation_plan::linearize_ops` では Swap と同じく状態変更 op を出さない。
- パレットは 2 段。1 段目は単量子ビットのユニタリ (H, X, Y, Z, √X, S, S†, T, T†, P, Rx, Ry, Rz)、2 段目は特殊ゲート (SWAP, •, ◦, |0⟩, |1⟩)。両段とも左寄せで揃える（qni の `flex flex-row` レイアウトに合わせる）。
- パレットの寸法は qni `apps/www/app/views/application/_palette_md.html.erb` に合わせる: ゲート間 8px (`space-x-2`)、行間 8px (`space-y-2`)、横パディング 16px (`px-4`)、縦パディング 20px (`py-5`)、角丸 12px (`rounded-xl`)。
- |0⟩ の桁は qni semantic-color-off (red-500: `#ef4444`)、|1⟩ の桁は semantic-color-on (blue-500: `#3b82f6`) で描画する。
- CNOT is expressed by placing a control gate (C) and an X gate in the same column.
- Control and anti-control gates apply to every non-control gate in the same column (same step).
- ドラッグ中は `needs_recompute` を立てず、状態ベクトルの再計算は drop/snap 時のみ実行する。
- ドラッグ中の state_count は `drag_state_count` で固定し、状態ベクトルの長さを変えない。
- 状態ベクトルのインスタンスは layout/offset が変わらない限りキャッシュし、GPU バッファ更新を抑制する。
- ドラッグ中の再描画は CooldownThrottle 相当で、10ms ベース + 0.1 倍ポンプ（Quirk 相当）で `request_repaint` と `request_repaint_after` を切り替える。
- ドラッグ中は回路側の影や接続線などの周辺装飾を省略して tessellator 負荷を下げる。一方で、いま掴んでいるゲート自身・回路上に既に置かれているゲート・パレット上のゲートは、ドラッグ中も通常描画（角丸・アイコン・ラベル維持）のままにする。
- いま掴んでいるゲートは qni の grabbed state に合わせて intermediate purple (`#a855f7`) で描画し、drop 後は通常の primary green に戻す。
- パレットから掴んだゲートのドラッグプレビューは、パレット panel と状態ベクトルウィンドウの両方より前面に描画して、重なっても隠れないようにする。
- ドラッグ中の最終カーソル位置は `drag_cursor_pos` を保持し、ドロップ時に位置が欠けないようにする。
- 起動直後は短時間だけ `request_repaint_after` を回してキャンバス描画を安定させる。
- ドラッグ遅延のプロファイル結果: `docs/egui-web-drag-profiling.md`。
- ドラッグ高速化の方針（Quirk 参考）: `docs/egui-web-drag-optimization-plan.md`。
