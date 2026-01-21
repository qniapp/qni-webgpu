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
