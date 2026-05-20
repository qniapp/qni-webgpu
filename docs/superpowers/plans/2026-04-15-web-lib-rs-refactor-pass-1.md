# web lib.rs refactor (pass 1) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the obvious low-risk seams from `apps/web/src/lib.rs` into `layout.rs` and `icons.rs` without changing web behavior.

**Architecture:** This pass is a behavior-preserving code-motion refactor only. Move layout/snap helpers into `apps/web/src/layout.rs` and gate drawing helpers into `apps/web/src/icons.rs`, leaving `QniApp`, the update loop, and GPU/shader code in `lib.rs` for now so the runtime glue stays stable.

**Tech Stack:** Rust, eframe/egui, wgpu/WebGPU, Playwright, trunk

---

## File structure for this pass

- Create: `apps/web/src/layout.rs`
  - Own `LayoutMetrics` and the slot/line/snap helper functions.
  - Accept the existing light dependency on `PlacedGate`; do not redesign it away in this pass.
- Create: `apps/web/src/icons.rs`
  - Own `SvgPoint`, SVG path helpers, and gate body/icon drawing helpers.
- Modify: `apps/web/src/lib.rs`
  - Add `mod layout; mod icons;`
  - Import the moved helpers with the minimum visibility needed (`pub(crate)` as required); do not re-export unless strictly necessary to keep compilation working.
  - Keep `QniApp`, `draw_circuit`, `draw_palette`, state panel drawing, and all GPU/shader code here.
- Verify: `apps/web/tests/web.spec.js`
  - Do not modify in this pass. Existing tests are the safety net for this behavior-preserving extraction.
- Reference spec: `docs/superpowers/specs/2026-04-15-web-lib-rs-refactor-design.md`

## Guardrails

- Move listed functions/types verbatim unless an import path or visibility fix is strictly required.
- Do not change constants, drawing parameters, thresholds, or control flow.
- Do not bundle GPU extraction, `QniApp` restructuring, or naming cleanups into this pass.
- If a step reveals a real behavior change, stop and repair it before moving to the next task.

### Task 1: Extract `layout.rs`

**Files:**
- Create: `apps/web/src/layout.rs`
- Modify: `apps/web/src/lib.rs`
- Reference: `docs/superpowers/specs/2026-04-15-web-lib-rs-refactor-design.md`

- [ ] **Step 1: Run the smallest baseline checks before moving code**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check
```
Expected: `Finished`/`Checking` completes successfully before code motion.

- [ ] **Step 2: Add the new module shell and declaration**

Create `apps/web/src/layout.rs` and add `mod layout;` near the top of `apps/web/src/lib.rs`.

Seed the new file with the moved symbol list:
```rust
use eframe::egui;

use crate::{DragState, GateKind, PlacedGate, GATE_SIZE, LINE_GAP, LINE_LEFT_OFFSET, LINE_RIGHT_OFFSET, LINE_Y, SLOT_SPACING};

#[derive(Clone, Debug)]
pub(crate) struct LayoutMetrics {
    pub(crate) line_left: f32,
    pub(crate) line_right: f32,
    pub(crate) line_ys: Vec<f32>,
    pub(crate) slot_left: f32,
    pub(crate) slot_right: f32,
    pub(crate) slot_centers: Vec<f32>,
}
```

- [ ] **Step 3: Move the layout helpers verbatim**

Move these definitions out of `lib.rs` into `layout.rs` with only the minimum visibility changes needed:
- `LayoutMetrics`
- `layout_metrics`
- `nearest_slot_center`
- `nearest_slot_index`
- `nearest_available_slot`
- `nearest_line`

Do **not** move `should_use_fast_gate_body` yet; keep it in `lib.rs` for this pass.

- [ ] **Step 4: Fix imports and compile immediately**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check
```
Expected: success with no unresolved module/symbol errors.

- [ ] **Step 5: Run a focused browser regression slice**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'dragging does not grow state vector until drop|CNOT with control on q1 yields bell state|Control does not affect gates in other columns'
```
Expected: all matched tests pass.

- [ ] **Step 6: Commit the layout extraction**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/layout.rs
git commit -m "refactor: extract web layout helpers"
```

### Task 2: Extract `icons.rs`

**Files:**
- Create: `apps/web/src/icons.rs`
- Modify: `apps/web/src/lib.rs`
- Verify: `apps/web/tests/web.spec.js`

- [ ] **Step 1: Run the icon-focused regression contract before moving code**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'palette panel keeps its corners and shadow while dragging|palette control gate keeps its icon while dragging|dragged palette gate keeps rounded corners|dragged x gate keeps the same visual as after drop'
```
Expected: all matched tests pass.

- [ ] **Step 2: Add the new module shell and declaration**

Create `apps/web/src/icons.rs` and add `mod icons;` near the top of `apps/web/src/lib.rs`.

Seed the new file with the shared helper imports:
```rust
use eframe::egui;

use crate::{Colors, GateKind};
```

- [ ] **Step 3: Move the SVG and gate drawing helpers verbatim**

Move these definitions out of `lib.rs` into `icons.rs`:
- `SvgPoint`
- `map_svg_point_in_rect`
- `push_cubic_points_viewbox`
- `draw_gate_body`
- `draw_gate_body_fast`
- `draw_gate_icon`
- `draw_r_letter`
- `draw_s_curve`

Keep function bodies unchanged except for import/visibility fixes.

- [ ] **Step 4: Fix imports and compile immediately**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check
```
Expected: success with no unresolved symbol errors.

- [ ] **Step 5: Re-run the icon-focused regression slice**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test --grep 'palette panel keeps its corners and shadow while dragging|palette control gate keeps its icon while dragging|dragged palette gate keeps rounded corners|dragged x gate keeps the same visual as after drop'
```
Expected: all matched tests pass again after extraction.

- [ ] **Step 6: Commit the icon extraction**

Run:
```bash
git add /home/yasuhito/Work/qni-webgpu/apps/web/src/lib.rs /home/yasuhito/Work/qni-webgpu/apps/web/src/icons.rs
git commit -m "refactor: extract web gate drawing helpers"
```

### Task 3: Full verification and choose the next split

**Files:**
- Modify: none required unless verification reveals a regression
- Reference: `apps/web/src/lib.rs`
- Reference: `docs/superpowers/specs/2026-04-15-web-lib-rs-refactor-design.md`

- [ ] **Step 1: Run the pass-1 scoped web verification**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && trunk build
```
Expected: all three commands pass.

- [ ] **Step 2: Run whitespace / patch hygiene verification**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```
Expected: no output.

- [ ] **Step 3: Verify that only the intended symbols moved**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n '^(pub\(crate\) )?(struct LayoutMetrics|fn layout_metrics|fn nearest_slot_center|fn nearest_slot_index|fn nearest_available_slot|fn nearest_line|struct SvgPoint|fn map_svg_point_in_rect|fn push_cubic_points_viewbox|fn draw_gate_body|fn draw_gate_body_fast|fn draw_gate_icon|fn draw_r_letter|fn draw_s_curve)' apps/web/src/lib.rs apps/web/src/layout.rs apps/web/src/icons.rs
```
Expected:
- the listed layout symbols appear only in `layout.rs`
- the listed icon symbols appear only in `icons.rs`
- none of those definitions remain in `lib.rs`

- [ ] **Step 4: Measure the remaining file sizes**

Run:
```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/web/src/lib.rs apps/web/src/layout.rs apps/web/src/icons.rs
```
Expected: `lib.rs` is smaller than before; capture the counts in your notes/review request.

- [ ] **Step 5: Identify the next split using the agreed rule**

Use the remaining `lib.rs` contents to pick the next pass by **largest remaining responsibility**, breaking ties by **natural dependency direction**:
- GPU/shader/readback largest → `gpu.rs`
- Drawing methods largest → `render.rs`
- App/update/state glue largest → `app.rs`

Do not implement pass 2 yet; just record the recommendation.

- [ ] **Step 6: Request review of the pass-1 refactor**

Ask a reviewer to inspect:
- `apps/web/src/lib.rs`
- `apps/web/src/layout.rs`
- `apps/web/src/icons.rs`
- verification evidence from `cargo check`, full `pnpm exec playwright test`, `trunk build`, and `git diff --check`
- the next-pass recommendation from Step 5

Focus the review on: behavior preservation, natural module boundaries, and whether pass 2 should target `gpu.rs`, `render.rs`, or `app.rs`.

- [ ] **Step 7: Commit any review-driven follow-up (if needed), then summarize**

If the reviewer requires changes, make only the necessary fixes and commit them with a focused message. Then summarize:
- what moved into `layout.rs`
- what moved into `icons.rs`
- the new LOC counts
- the recommended pass-2 target
