# Ralph Progress Log - qni-webgpu
Started: 2026-01-08

## Codebase Patterns
- **Monorepo structure**: apps/, crates/, docs/
- **Lint**: `cargo clippy -- -D warnings`
- **Check all**: `./scripts/check-all.sh`

## Key Projects
- `apps/egui-web` - egui + WebGPU frontend (Trunk, Playwright tests)
- `apps/tui` - Terminal UI

## Build Commands
- egui-web: `cd apps/egui-web && trunk serve`
- tui: `cd apps/tui && cargo run`

- **WASM clippy**: Run `cargo clippy --target wasm32-unknown-unknown -- -D warnings` (native target fails on this codebase)
- **Color constants**: Colors are defined in `Colors` struct in `apps/egui-web/src/lib.rs` using normalized floats (0.0-1.0)
- **Reference colors**: The qni project uses Tailwind CSS colors - see `packages/elements/src/qubit-circle-element.ts`

---
## Progress Entries
(Ralph will append entries below as stories are completed)

---
## 2026-01-08 - US-001
- Changed state vector circle fill color to Tailwind sky-500 (rgb 14, 165, 233)
- **Files changed**: `apps/egui-web/src/lib.rs` (line 2117)
- **Learnings:**
  - Color conversion: RGB 0-255 to normalized float is `value / 255` (e.g., 14/255 ≈ 0.055)
  - sky-500 = `color_rgba(0.055, 0.647, 0.914, 1.0)`
  - Previous color was too dark: `color_rgba(0.16, 0.58, 0.78, 1.0)`
  - Use `trunk build` for WASM builds, not native cargo build

