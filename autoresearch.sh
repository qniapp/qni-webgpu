#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKFLOW_PATH="$ROOT_DIR/.github/workflows/ci.yml"

bash -n "$ROOT_DIR/scripts/check-all.sh"

measure_seconds() {
  local label="$1"
  shift

  local start_ns end_ns
  start_ns=$(python - <<'PY'
import time
print(time.perf_counter_ns())
PY
)

  "$@" >/tmp/"${label}".log 2>&1

  end_ns=$(python - <<'PY'
import time
print(time.perf_counter_ns())
PY
)

  python - "$start_ns" "$end_ns" <<'PY'
import sys
start_ns = int(sys.argv[1])
end_ns = int(sys.argv[2])
print(f"{(end_ns - start_ns) / 1_000_000_000:.3f}")
PY
}

WEB_PREFLIGHT_S=$(measure_seconds web-preflight pnpm -C "$ROOT_DIR/apps/egui-web" run test:preflight)
WEB_BDD_S=$(measure_seconds web-bdd pnpm -C "$ROOT_DIR/apps/egui-web" run test:bdd)
WEB_LEGACY_S=$(measure_seconds web-legacy pnpm -C "$ROOT_DIR/apps/egui-web" run test:pw-legacy)
MCP_CHECK_S=$(measure_seconds mcp-check pnpm -C "$ROOT_DIR/apps/mcp-qni" check)
TUI_CHECK_S=$(measure_seconds tui-check make -C "$ROOT_DIR" check)

WEB_LOCAL_S=$(python - "$WEB_PREFLIGHT_S" "$WEB_BDD_S" "$WEB_LEGACY_S" <<'PY'
import sys
print(f"{sum(float(v) for v in sys.argv[1:]):.3f}")
PY
)

ruby - "$WORKFLOW_PATH" "$WEB_PREFLIGHT_S" "$WEB_BDD_S" "$WEB_LEGACY_S" "$MCP_CHECK_S" "$TUI_CHECK_S" <<'RUBY'
require 'yaml'

workflow_path, web_preflight_s, web_bdd_s, web_legacy_s, mcp_check_s, tui_check_s = ARGV
config = YAML.load_file(workflow_path)
jobs = config.fetch('jobs')

setup_cost_s = {
  'Set up job' => 3.0,
  'Checkout' => 1.0,
  'Setup Node' => 1.0,
  'Setup pnpm' => 1.0,
  'Setup Rust' => 10.0,
  'Install wasm32 target' => 2.0,
  'Install trunk' => 438.0,
  'Install cargo audit/deny' => 458.0,
  'Install Web deps' => 3.0,
  'Install Playwright browser' => 13.0,
  'Install MCP deps' => 2.0,
}

command_cost_s = {
  'pnpm -C apps/egui-web run test:preflight' => web_preflight_s.to_f,
  'pnpm -C apps/egui-web run test:bdd' => web_bdd_s.to_f,
  'pnpm -C apps/egui-web run test:pw-legacy' => web_legacy_s.to_f,
  'pnpm -C apps/mcp-qni check' => mcp_check_s.to_f,
  'make -C . check' => tui_check_s.to_f,
}

web_bundle = web_preflight_s.to_f + web_bdd_s.to_f + web_legacy_s.to_f
full_bundle = web_bundle + mcp_check_s.to_f + tui_check_s.to_f

normalize_run = lambda do |run_text, working_directory|
  text = run_text.to_s.strip.gsub(/\s+/, ' ')
  wd = working_directory.to_s.strip
  return nil if text.empty?

  if text == './scripts/check-all.sh'
    return full_bundle
  end

  return web_preflight_s.to_f if text.include?('test:preflight')
  return web_bdd_s.to_f if text.include?('test:bdd')
  return web_legacy_s.to_f if text.include?('test:pw-legacy') || (wd.include?('apps/egui-web') && text == 'playwright test')
  return mcp_check_s.to_f if text.include?('apps/mcp-qni') && text.include?('check')
  return mcp_check_s.to_f if wd.include?('apps/mcp-qni') && text == 'pnpm check'
  return tui_check_s.to_f if text.include?('make -C') && text.include?('check')
  return tui_check_s.to_f if text == 'make check'

  nil
end

job_totals = {}
unknown_steps = []

jobs.each do |job_name, job|
  total_s = 0.0
  Array(job['steps']).each do |step|
    name = step['name'].to_s
    if setup_cost_s.key?(name)
      total_s += setup_cost_s.fetch(name)
      next
    end

    run_cost = normalize_run.call(step['run'], step['working-directory'])
    if run_cost
      total_s += run_cost
      next
    end

    next if name.start_with?('Post ')
    next if name == 'Complete job'

    unknown_steps << [job_name, name, step['run'].to_s.strip]
  end
  job_totals[job_name] = total_s
end

projected_ci_s = job_totals.values.max || 0.0

puts "METRIC projected_ci_s=#{projected_ci_s.round(3)}"
puts "METRIC web_local_s=#{web_bundle.round(3)}"
puts "METRIC mcp_local_s=#{mcp_check_s.to_f.round(3)}"
puts "METRIC tui_local_s=#{tui_check_s.to_f.round(3)}"
puts "METRIC modeled_jobs=#{job_totals.size}"
puts "METRIC modeled_unknown_steps=#{unknown_steps.size}"
unknown_steps.each do |job_name, step_name, run_text|
  warn "UNMODELED_STEP #{job_name} :: #{step_name} :: #{run_text}"
end
RUBY
