# Autoresearch: GitHub Actions CI runtime reduction

## Objective
Reduce qni-webgpu GitHub Actions wall-clock time without dropping any existing validation coverage.

The current successful baseline is Actions run `25151678429` on `master`, which took **18m53s** end-to-end. The dominant costs in that run were:
- `Install trunk`: 438s
- `Install cargo audit/deny`: 458s
- `Run all checks`: 197s

Because each real CI run is expensive, this session uses a **projected CI critical-path proxy** for rapid iteration:
1. measure the current local validation command durations,
2. parse `.github/workflows/ci.yml`,
3. assign fixed setup costs from the latest successful CI run,
4. compute the modeled per-job wall time and use the **max(job time)** as the primary metric.

Final acceptance still requires a real GitHub Actions run.

## Metrics
- **Primary**: `projected_ci_s` (s, lower is better) — modeled GitHub Actions critical-path wall time
- **Secondary**:
  - `web_local_s`
  - `mcp_local_s`
  - `tui_local_s`
  - `modeled_jobs`
  - `modeled_unknown_steps`

## How to Run
`./autoresearch.sh`

The script outputs `METRIC ...` lines for the projected CI wall time and local component timings.

## Files in Scope
- `.github/workflows/ci.yml` — CI topology, setup, caching, and parallelization
- `scripts/check-all.sh` — only if needed to preserve or restructure CI validation flow
- `autoresearch.md`
- `autoresearch.sh`
- `autoresearch.jsonl`
- `autoresearch.ideas.md`

## Off Limits
- App/runtime source files under `apps/**/src`
- Test semantics or coverage reduction
- Browser launch policy / WebGPU behavior changes unrelated to CI duration
- Cargo/Rust dependency graph changes unless strictly required for CI tooling setup

## Constraints
- Keep existing validation coverage: Web preflight, Web BDD, Web legacy Playwright, MCP check, and TUI `make check`
- No dropping steps just to improve the metric
- Prefer CI-structure and install/caching optimizations over product-code changes
- Final winning candidate must pass fresh local verification and then a real GitHub Actions run
- Keep changes readable and maintainable; avoid opaque workflow tricks

## What's Been Tried
- Baseline success run `25151678429`: 18m53s total, with setup dominating more than app checks.
- CI flake fixes for web Cucumber were completed before this session; this lane is now focused on runtime only.
- Baseline proxy measurement on the current single-job workflow: `projected_ci_s=956.375`.
- Winning experiment so far: split `.github/workflows/ci.yml` into parallel `web`, `mcp`, and `tui` jobs while preserving the exact validation commands and enabling `setup-node` pnpm cache + `pnpm install --frozen-lockfile` in the Node jobs.
  - Initial proxy result: `projected_ci_s=486.499`.
  - After review-driven fix (run `pnpm/action-setup` before `actions/setup-node` in the Node jobs): `projected_ci_s=484.554`.
- Reviewer found one important safety issue in the first draft: `setup-node` pnpm caching must come **after** `pnpm/action-setup`, or cache restore can fail because `pnpm` is not available yet.
- Next high-upside lane, not yet explored in code: reduce the remaining dominant setup costs (`Install trunk`, `Install cargo audit/deny`) with prebuilt-binary installation and/or Rust caching.
