# Ralph Progress Log - qni-webgpu
Started: 2026-01-08

## Codebase Patterns
- **Monorepo structure**: `apps/`, `docs/`, `scripts/`
- **Primary apps**: `apps/egui-web`, `apps/tui`, `apps/mcp-qni`
- **Top-level verification**: `./scripts/check-all.sh`
- **TUI lint**: `cd apps/tui && cargo clippy -- -D warnings`
- **egui-web prerequisites**: `rustup target add wasm32-unknown-unknown` and `cargo install trunk --locked`
- **確認用サーバ**: Web 側の確認では `trunk serve` を使う

## Key Projects
- `apps/egui-web` - egui + WebGPU frontend (Trunk, Playwright tests)
- `apps/tui` - Terminal UI (ratatui, snapshot/E2E tests)
- `apps/mcp-qni` - MCP server for circuit editing and state-vector execution

## Build Commands
- egui-web: `cd apps/egui-web && trunk serve --address 127.0.0.1 --port 4174 --no-autoreload`
- egui-web test: `cd apps/egui-web && pnpm exec playwright test`
- tui: `cd apps/tui && cargo run`
- mcp-qni: `cd apps/mcp-qni && pnpm start`

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

---
## 2026-01-08 - US-004
- Changed wire label to endpoint gap to 0.5rem (16px)
- **Files changed**: `apps/egui-web/src/lib.rs` (line 22)
- **Learnings:**
  - `QUBIT_LABEL_GAP` controls spacing between wire labels (q0:, q1:) and wire start points
  - Changed from hardcoded `12.0` to `0.5 * REM` for consistent rem-based spacing
  - REM = 32.0 in this codebase, so 0.5 * REM = 16.0 pixels
  - Layout constants are at the top of lib.rs (lines 8-32)


## Review
- What's correct: README / docs / MCP docs are aligned with current code and config for tool availability, gate sets, and check commands.
- Fixed: `progress.md` の egui-web 実行コマンドを現行設定（`--address ... --no-autoreload`）に合わせた。
- Fixed: `docs/tui.md` のコードフェンス崩れを修正し、手順セクションが正しくレンダリングされるようにした。
- Note: 上記以外の対象ドキュメントは、現行コード/設定との事実不整合は確認されなかった。
