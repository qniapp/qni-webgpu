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
- The Playwright test drags the H gate onto q0, waits for `window.__eguiReadStateVector()` to match the expected amplitudes, and checks that the canvas contains non-background pixels.
- The Playwright run writes a screenshot to `/tmp/qni-egui-webgpu.png`.
