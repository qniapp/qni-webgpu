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

ruby - "$ROOT_DIR" "$WORKFLOW_PATH" "$WEB_PREFLIGHT_S" "$WEB_BDD_S" "$WEB_LEGACY_S" "$MCP_CHECK_S" "$TUI_CHECK_S" <<'RUBY'
require 'json'
require 'open3'
require 'time'
require 'yaml'

root_dir, workflow_path, web_preflight_s, web_bdd_s, web_legacy_s, mcp_check_s, tui_check_s = ARGV
config = YAML.load_file(workflow_path)
jobs = config.fetch('jobs')

fallback_step_cost_s = {
  'Set up job' => 3.0,
  'Checkout' => 1.0,
  'Setup Node' => 1.0,
  'Setup pnpm' => 1.0,
  'Setup Rust' => 10.0,
  'Cache Rust artifacts' => 0.0,
  'Install wasm32 target' => 2.0,
  'Install trunk' => 438.0,
  'Install cargo audit/deny' => 458.0,
  'Install Web deps' => 3.0,
  'Install Playwright browser' => 13.0,
  'Install MCP deps' => 2.0,
  'Web trunk build' => 55.0,
  'Upload web dist artifact' => 2.0,
  'Download web dist artifact' => 2.0,
  'Web Cucumber BDD (static dist)' => 15.0,
  'Web Playwright legacy (static dist)' => 45.0,
}

web_bundle = web_preflight_s.to_f + web_bdd_s.to_f + web_legacy_s.to_f
full_bundle = web_bundle + mcp_check_s.to_f + tui_check_s.to_f

command_cost_s = {
  'pnpm -C apps/egui-web run test:preflight' => web_preflight_s.to_f,
  'pnpm -C apps/egui-web run test:bdd' => web_bdd_s.to_f,
  'pnpm -C apps/egui-web run test:pw-legacy' => web_legacy_s.to_f,
  'pnpm -C apps/mcp-qni check' => mcp_check_s.to_f,
  'make -C . check' => tui_check_s.to_f,
}

branch_name = ENV['AUTORESEARCH_BRANCH']
if branch_name.to_s.empty?
  out, status = Open3.capture2('git', '-C', root_dir, 'rev-parse', '--abbrev-ref', 'HEAD')
  branch_name = out.strip if status.success?
end

observed_job_step_cost_s = {}
observed_step_cost_lists = Hash.new { |hash, key| hash[key] = [] }
observed_run_id = nil
observed_run_count = 0

if !branch_name.to_s.empty?
  run_list_out, run_list_status = Open3.capture2(
    'gh', 'run', 'list',
    '--branch', branch_name,
    '--workflow', 'CI',
    '--limit', '20',
    '--json', 'databaseId,conclusion'
  )

  if run_list_status.success?
    successful_runs = JSON.parse(run_list_out).select { |run| run['conclusion'] == 'success' }
    observed_run_id = successful_runs.first&.fetch('databaseId', nil)

    successful_runs.first(5).each_with_index do |run, index|
      run_view_out, run_view_status = Open3.capture2(
        'gh', 'run', 'view', run.fetch('databaseId').to_s,
        '--json', 'jobs'
      )
      next unless run_view_status.success?

      observed_run_count += 1
      JSON.parse(run_view_out).fetch('jobs').each do |job|
        Array(job['steps']).each do |step|
          next unless step['status'] == 'completed'
          next if step['name'].to_s.start_with?('Post ')
          next if step['name'].to_s == 'Complete job'

          started_at = step['startedAt']
          completed_at = step['completedAt']
          next if started_at.to_s.empty? || completed_at.to_s.empty?

          duration_s = Time.iso8601(completed_at) - Time.iso8601(started_at)
          observed_job_step_cost_s[[job['name'], step['name']]] = duration_s if index.zero?
          observed_step_cost_lists[step['name']] << duration_s
        end
      end
    end
  end
end

observed_step_cost_s = observed_step_cost_lists.transform_values do |durations|
  durations.sum / durations.size
end
step_cost_s = fallback_step_cost_s.merge(observed_step_cost_s)

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

    run_cost = normalize_run.call(step['run'], step['working-directory'])
    if run_cost
      total_s += run_cost
      next
    end

    if observed_job_step_cost_s.key?([job_name, name])
      total_s += observed_job_step_cost_s.fetch([job_name, name])
      next
    end

    if step_cost_s.key?(name)
      total_s += step_cost_s.fetch(name)
      next
    end

    next if name.start_with?('Post ')
    next if name == 'Complete job'

    unknown_steps << [job_name, name, step['run'].to_s.strip]
  end
  job_totals[job_name] = total_s
end

path_totals = {}
visiting = {}
critical_path = lambda do |job_name|
  return path_totals.fetch(job_name) if path_totals.key?(job_name)
  raise "cyclic job dependency at #{job_name}" if visiting[job_name]

  visiting[job_name] = true
  job = jobs.fetch(job_name)
  dependencies = Array(job['needs']).map(&:to_s)
  dependency_total = dependencies.map { |dependency| critical_path.call(dependency) }.max || 0.0
  path_totals[job_name] = dependency_total + job_totals.fetch(job_name, 0.0)
  visiting.delete(job_name)
  path_totals.fetch(job_name)
end

projected_ci_s = jobs.keys.map { |job_name| critical_path.call(job_name) }.max || 0.0

puts "METRIC projected_ci_s=#{projected_ci_s.round(3)}"
puts "METRIC web_local_s=#{web_bundle.round(3)}"
puts "METRIC mcp_local_s=#{mcp_check_s.to_f.round(3)}"
puts "METRIC tui_local_s=#{tui_check_s.to_f.round(3)}"
puts "METRIC modeled_jobs=#{job_totals.size}"
puts "METRIC modeled_unknown_steps=#{unknown_steps.size}"
puts "METRIC observed_ci_run_id=#{observed_run_id || 0}"
puts "METRIC observed_ci_run_count=#{observed_run_count}"
unknown_steps.each do |job_name, step_name, run_text|
  warn "UNMODELED_STEP #{job_name} :: #{step_name} :: #{run_text}"
end
RUBY
