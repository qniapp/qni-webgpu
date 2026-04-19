# egui WebGPU PoC (Rust)

## Prerequisites
- `rustup target add wasm32-unknown-unknown`
- `cargo install trunk`

## Run (local)
まずサーバを起動する。
```
cd apps/egui-web
trunk serve --address 127.0.0.1 --port 4174
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

## Playwright
```
cd apps/egui-web
pnpm install
pnpm exec playwright install chromium
pnpm exec playwright test
```
MCP から Playwright を使う場合は、helper の `scripts/playwright-mcp.sh` を使うと
`.playwright-mcp/config.json` を自動検出しつつ `--isolated` 付きで起動できる。
（このプロジェクトの `.mcp.json` でも同等の設定を直接記述している。）
WebGPU は X がないと adapter が取れないため、Xvfb を挟んで実行する。
```
cd apps/egui-web
xvfb-run -a -s "-screen 0 1920x1080x24" pnpm exec playwright test
```
`@playwright/test` の headless shell だと SRI mismatch が起きるため、
`playwright.config.cjs` は Playwright 同梱ブラウザよりも先に `google-chrome-stable` を優先し、
見つからない場合のみ `chromium.executablePath()` に fallback する。
必要なら `PLAYWRIGHT_CHROMIUM_PATH` で明示上書きできる。
`pnpm run test:preflight` では、この browser 選択ロジックの Node テストだけを先に確認できる。

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
- パレットから掴んだゲートのドラッグプレビューは、パレット panel より前面に描画して隠れないようにする（state panel の前後関係は現状維持）。
- ドラッグ中の最終カーソル位置は `drag_cursor_pos` を保持し、ドロップ時に位置が欠けないようにする。
- 起動直後は短時間だけ `request_repaint_after` を回してキャンバス描画を安定させる。
- ドラッグ遅延のプロファイル結果: `docs/egui-web-drag-profiling.md`。
- ドラッグ高速化の方針（Quirk 参考）: `docs/egui-web-drag-optimization-plan.md`。
