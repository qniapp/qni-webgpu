# egui WebGPU PoC (Rust)

## Prerequisites
- `rustup target add wasm32-unknown-unknown`
- `cargo install trunk`

## Run (local)
```
cd apps/egui-web
trunk serve --host 127.0.0.1 --port 4174 --no-open
```
Open: `http://127.0.0.1:4174/`

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
`playwright.config.cjs` は `chromium.executablePath()` を使って full Chromium を起動する。

## Notes
- `apps/egui-web/src/lib.rs` uses eframe with the `wgpu` feature enabled.
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
- ドラッグ中は gate/palette を簡略描画（角丸・影・アイコン線を省略）して tessellator 負荷を下げる。
- ドラッグ中の最終カーソル位置は `drag_cursor_pos` を保持し、ドロップ時に位置が欠けないようにする。
- 起動直後は短時間だけ `request_repaint_after` を回してキャンバス描画を安定させる。
- ドラッグ遅延のプロファイル結果: `docs/egui-web-drag-profiling.md`。
- ドラッグ高速化の方針（Quirk 参考）: `docs/egui-web-drag-optimization-plan.md`。
