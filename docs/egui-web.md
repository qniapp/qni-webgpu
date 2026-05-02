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

Linux / Wayland では通常起動のブラウザだと WebGPU adapter を取れず、白画面や初期化エラーになることがある。
今後のローカル動作確認は、**フラグ付きの Google Chrome を正本**として扱う。
まずはリポジトリルートから helper script を使う。
```
./scripts/open-egui-web.sh
```
この script は `google-chrome-stable` を最優先で探し、見つからない場合のみ Chromium 系へ fallback する。
起動時には `--ozone-platform=x11` と WebGPU 用フラグを付ける。
`--disable-gpu-sandbox` は使わない。`--enable-unsafe-webgpu` による警告バーは表示されるが、現状のローカル実行では想定内。

明示的にブラウザを固定したい場合の例:
```
QNI_EGUI_WEB_BROWSER=/usr/bin/google-chrome-stable ./scripts/open-egui-web.sh
```

環境変数:
- `QNI_EGUI_WEB_BROWSER`: 使用する Chromium 系ブラウザを明示
- `QNI_EGUI_WEB_PORT`: 接続先ポートを変更
- `QNI_EGUI_WEB_URL`: 接続先 URL を直接指定
- `QNI_EGUI_WEB_PROFILE_DIR`: 一時 profile ディレクトリを変更

## Codex browser / visual workflow
Codex アプリ内ブラウザは `http://127.0.0.1:4174/` を開けるが、Chrome 起動フラグを付けられない。
WebGPU adapter が必要な実描画確認では、アプリ内ブラウザは URL 到達確認に留め、外部のフラグ付き Chrome か Playwright 経路を使う。

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

Codex からスクリーンショットや操作込みで確認する場合は、既存サーバを使って headed Playwright を走らせる。
この経路は `test-support/browser-launch.cjs` の WebGPU フラグ付き Chrome 設定を使う。
```
cd apps/egui-web
QNI_EGUI_WEB_EXTERNAL_SERVER=1 HEADLESS=0 pnpm exec playwright test --grep 'egui webgpu canvas renders content' --workers=1
```

Codex からゲートを意味ベースでドラッグアンドドロップする場合は、専用 CLI を使う。
この CLI は Playwright で Chrome を起動し、`H:q0:0` のような指定を palette gate → circuit slot のドラッグへ変換する。
目視確認用にページ全体 screenshot を保存し、`window.__eguiReadStateVector()` も JSON で出力する。
```
cd apps/egui-web
QNI_EGUI_WEB_EXTERNAL_SERVER=1 node scripts/codex-visual.cjs drag \
  --gate H --wire q0 --slot 0 \
  --out output/playwright/codex-visual/h-q0.png

QNI_EGUI_WEB_EXTERNAL_SERVER=1 node scripts/codex-visual.cjs ops \
  --ops H:q0:0,C:q0:1,X:q1:1 \
  --out output/playwright/codex-visual/bell.png
```

`scripts/codex-visual.cjs` は通常の `@playwright/test` 用 SwiftShader launch ではなく、screenshot が黒くならない Codex visual launch を使う。
現状の egui content margin に合わせて drop 座標に `--vertical-offset 8` を既定で加える。
UI の外枠や egui panel margin を変えた場合は、この値を一時的に上書きして確認する。
```
QNI_EGUI_WEB_EXTERNAL_SERVER=1 node scripts/codex-visual.cjs drag \
  --gate X --wire q1 --slot 2 \
  --vertical-offset 8
```

手動デバッグで DevTools/CDP 接続が必要なときだけ、専用 profile と remote debugging port を使って Chrome を起動する。
```
/usr/bin/google-chrome-stable \
  --user-data-dir=/tmp/qni-webgpu-chrome-codex-verify \
  --new-window \
  --no-first-run \
  --no-default-browser-check \
  --ozone-platform=x11 \
  --enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan \
  --enable-unsafe-webgpu \
  --ignore-gpu-blocklist \
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

Step definitions は段階的に TypeScript へ移行する。
`cucumber.cjs` は Cucumber v12 の `default` profile として定義し、`ts-node/register` を読み込む。
対象は `features/step_definitions/**/*.cjs` と `features/step_definitions/**/*.ts` の両方にする。
新規または移行済みの step は `.steps.ts` で書き、`pnpm run typecheck` と `pnpm run test:bdd` の両方で確認する。
移行中は同じ step phrase を CJS と TS の両方に残すと Cucumber の duplicate step になるため、1 file ずつ置き換える。
現在の 3 本の代表 scenario の step definitions は `.steps.ts` に移行済みで、共有する World 型は `features/support/world-types.ts` に置く。
既存の CJS support modules はそのまま使い、TS step からは `require(...) as ...` で薄く型を付ける。

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
`playwright.config.cjs` は Playwright 同梱ブラウザよりも先に `google-chrome-stable` を優先し、
見つからない場合のみ `chromium.executablePath()` に fallback する。
必要なら `PLAYWRIGHT_CHROMIUM_PATH` で明示上書きできる。
この browser policy と `trunk serve --address 127.0.0.1 --port 4174 --no-autoreload` の server policy は
`test-support/browser-launch.cjs` と `test-support/web-server.cjs` に集約されており、
legacy Playwright と BDD の両方が同じ shared source of truth を使う。
そのため、flagged Chrome を正本にする挙動は両経路で一致する。

repo root の `scripts/check-all.sh` でも staged rollout を維持し、
`test:preflight` → `test:bdd` → `test:pw-legacy` の順で Web の gate を通す。

## Notes
- `apps/egui-web/src/lib.rs` uses eframe with the `wgpu` feature enabled.
- 通常のブラウザ起動で利用可能な WebGPU adapter が見つからない場合、キャンバスが白いままになる代わりに、ページ上に WebGPU 初期化失敗メッセージを表示する。
- Linux / Wayland では Wayland + swiftshader 系の起動オプションで真っ黒になることがあり、現状は `./scripts/open-egui-web.sh` で起動するフラグ付き Google Chrome（fallback: Chromium）の X11 起動を正本とする。
- 状態ベクトルの計算と円描画は WebGPU（Compute/Fragment）で行い、CPU への読み戻しはテスト時のみ。
- `window.__eguiReadStateVector()` は非同期（Promise）で、Playwright は await して検証する。
- The Playwright test drags the H gate onto q0, waits for `window.__eguiReadStateVector()` to match the expected amplitudes, and checks that the canvas contains non-background pixels.
- The Playwright run writes screenshots to `/tmp/qni-egui-webgpu-initial.png` and `/tmp/qni-egui-webgpu-after.png`.
- Playwright uses `trunk serve --no-autoreload` to keep the canvas DOM stable during screenshots.
- State circles use shader-side AA (fwidth + smoothstep) for fill/outline/needle to reduce jagged edges.
- The circle quad now expands to include stroke width to avoid flat/clipped edges.
- The vertex quad adds a small pad (1px) so the AA fringe isn't clipped at the bounds.
- Compute dispatches submit per gate so each pass sees its own GateParams (avoids reusing the last params across multiple gates).
- CNOT is expressed by placing a control gate (C) and an X gate in the same column.
- Control gates apply to every non-control gate in the same column (same step).
- ドラッグ中は `needs_recompute` を立てず、状態ベクトルの再計算は drop/snap 時のみ実行する。
- ドラッグ中の state_count は `drag_state_count` で固定し、状態ベクトルの長さを変えない。
- 状態ベクトルのインスタンスは layout/offset が変わらない限りキャッシュし、GPU バッファ更新を抑制する。
- ドラッグ中の再描画は CooldownThrottle 相当で、10ms ベース + 0.1 倍ポンプ（Quirk 相当）で `request_repaint` と `request_repaint_after` を切り替える。
- ドラッグ中は回路側の影や接続線などの周辺装飾を省略して tessellator 負荷を下げる。一方で、いま掴んでいるゲート自身・回路上に既に置かれているゲート・パレット上のゲートは、ドラッグ中も通常描画（角丸・アイコン・ラベル維持）のままにする。
- パレットから掴んだゲートのドラッグプレビューは、パレット panel と状態ベクトルウィンドウの両方より前面に描画して、重なっても隠れないようにする。
- ドラッグ中の最終カーソル位置は `drag_cursor_pos` を保持し、ドロップ時に位置が欠けないようにする。
- 起動直後は短時間だけ `request_repaint_after` を回してキャンバス描画を安定させる。
- ドラッグ遅延のプロファイル結果: `docs/egui-web-drag-profiling.md`。
- ドラッグ高速化の方針（Quirk 参考）: `docs/egui-web-drag-optimization-plan.md`。
