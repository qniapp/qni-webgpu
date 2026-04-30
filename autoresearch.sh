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
WEB_TRUNK_BUILD_S=$(measure_seconds web-trunk-build bash -lc "cd '$ROOT_DIR/apps/egui-web' && env -u NO_COLOR TRUNK_COLOR=never trunk build")
WEB_TRUNK_BUILD_COLD_S=$(measure_seconds web-trunk-build-cold bash -lc "cd '$ROOT_DIR/apps/egui-web' && tmp_cache=\$(mktemp -d) && trap 'rm -rf \"\$tmp_cache\"' EXIT && XDG_CACHE_HOME=\$tmp_cache env -u NO_COLOR TRUNK_COLOR=never trunk build")
WEB_TRUNK_BUILD_COLD_SYSTEM_WASM_BINDGEN_S=$(measure_seconds web-trunk-build-cold-system-wasm-bindgen bash -lc "cd '$ROOT_DIR/apps/egui-web' && tmp_cache=\$(mktemp -d) && candidate_dir=\$(python - <<'PY'
from pathlib import Path
matches = sorted(Path.home().glob('.cache/trunk/wasm-bindgen-*/wasm-bindgen'))
print(matches[-1].parent if matches else '')
PY
) && trap 'rm -rf \"\$tmp_cache\"' EXIT && if [ -n \"\$candidate_dir\" ]; then PATH=\"\$candidate_dir:\$PATH\" XDG_CACHE_HOME=\$tmp_cache env -u NO_COLOR TRUNK_COLOR=never trunk build; else XDG_CACHE_HOME=\$tmp_cache env -u NO_COLOR TRUNK_COLOR=never trunk build; fi")
WEB_BDD_S=$(measure_seconds web-bdd bash -lc "cd '$ROOT_DIR/apps/egui-web' && python3 -m http.server 4174 --bind 127.0.0.1 --directory dist >/tmp/egui-web-bdd-benchmark.log 2>&1 & server_pid=\$! && trap 'kill \"\$server_pid\" 2>/dev/null || true' EXIT && CI=1 QNI_EGUI_WEB_EXTERNAL_SERVER=1 QNI_EGUI_WEB_BASE_URL=http://127.0.0.1:4174 pnpm run test:bdd")
WEB_LEGACY_S=$(measure_seconds web-legacy bash -lc "cd '$ROOT_DIR/apps/egui-web' && python3 -m http.server 4174 --bind 127.0.0.1 --directory dist >/tmp/egui-web-legacy-benchmark.log 2>&1 & server_pid=\$! && trap 'kill \"\$server_pid\" 2>/dev/null || true' EXIT && CI=1 QNI_EGUI_WEB_EXTERNAL_SERVER=1 QNI_EGUI_WEB_BASE_URL=http://127.0.0.1:4174 pnpm run test:pw-legacy")
MCP_CHECK_S=$(measure_seconds mcp-check pnpm -C "$ROOT_DIR/apps/mcp-qni" check)
TUI_CHECK_S=$(measure_seconds tui-check make -C "$ROOT_DIR" check)

WEB_LOCAL_S=$(python - "$WEB_PREFLIGHT_S" "$WEB_BDD_S" "$WEB_LEGACY_S" <<'PY'
import sys
print(f"{sum(float(v) for v in sys.argv[1:]):.3f}")
PY
)

ruby - "$ROOT_DIR" "$WORKFLOW_PATH" "$WEB_PREFLIGHT_S" "$WEB_TRUNK_BUILD_S" "$WEB_TRUNK_BUILD_COLD_S" "$WEB_TRUNK_BUILD_COLD_SYSTEM_WASM_BINDGEN_S" "$WEB_BDD_S" "$WEB_LEGACY_S" "$MCP_CHECK_S" "$TUI_CHECK_S" <<'RUBY'
require 'json'
require 'open3'
require 'time'
require 'yaml'

root_dir, workflow_path, web_preflight_s, web_trunk_build_s, web_trunk_build_cold_s, web_trunk_build_cold_system_wasm_bindgen_s, web_bdd_s, web_legacy_s, mcp_check_s, tui_check_s = ARGV
config = YAML.load_file(workflow_path)
jobs = config.fetch('jobs')

fallback_step_cost_s = {
  'Set up job' => 3.0,
  'Checkout' => 1.0,
  'Setup Node' => 1.0,
  'Setup pnpm' => 1.0,
  'Setup Rust' => 10.0,
  'Cache Rust artifacts' => 0.0,
  'Cache Trunk tools' => 1.0,
  'Install wasm32 target' => 2.0,
  'Install trunk' => 438.0,
  'Install wasm-bindgen' => 1.0,
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
observed_job_step_cost_lists = Hash.new { |hash, key| hash[key] = [] }
observed_step_cost_lists = Hash.new { |hash, key| hash[key] = [] }
observed_run_id = nil
observed_run_count = 0

expected_job_steps = jobs.transform_values do |job|
  Array(job['steps']).map { |step| step['name'].to_s }
end

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
    run_records = []

    successful_runs.each do |run|
      run_view_out, run_view_status = Open3.capture2(
        'gh', 'run', 'view', run.fetch('databaseId').to_s,
        '--json', 'jobs'
      )
      next unless run_view_status.success?

      run_jobs = JSON.parse(run_view_out).fetch('jobs')
      run_job_steps = run_jobs.each_with_object({}) do |job, hash|
        hash[job['name']] = Array(job['steps']).filter_map do |step|
          next unless step['status'] == 'completed'
          next if step['name'].to_s == 'Set up job'
          next if step['name'].to_s.start_with?('Post ')
          next if step['name'].to_s == 'Complete job'

          step['name'].to_s
        end
      end

      matches_current_workflow =
        run_job_steps.keys.sort == expected_job_steps.keys.sort &&
        expected_job_steps.all? { |job_name, steps| run_job_steps[job_name] == steps }

      run_records << {
        id: run.fetch('databaseId'),
        jobs: run_jobs,
        matching: matches_current_workflow,
      }
    end

    matching_records = run_records.select { |record| record[:matching] }
    selected_records = matching_records.empty? ? run_records.first(5) : matching_records.first(5)
    observed_run_id = selected_records.first&.fetch(:id, nil)
    observed_run_count = selected_records.size

    selected_records.each do |record|
      record.fetch(:jobs).each do |job|
        Array(job['steps']).each do |step|
          next unless step['status'] == 'completed'
          next if step['name'].to_s.start_with?('Post ')
          next if step['name'].to_s == 'Complete job'

          started_at = step['startedAt']
          completed_at = step['completedAt']
          next if started_at.to_s.empty? || completed_at.to_s.empty?

          duration_s = Time.iso8601(completed_at) - Time.iso8601(started_at)
          observed_job_step_cost_lists[[job['name'], step['name']]] << duration_s
          observed_step_cost_lists[step['name']] << duration_s
        end
      end
    end
  end
end

median = lambda do |durations|
  sorted = durations.sort
  middle = sorted.length / 2
  if sorted.length.odd?
    sorted[middle]
  else
    (sorted[middle - 1] + sorted[middle]) / 2.0
  end
end

observed_job_step_cost_s = observed_job_step_cost_lists.transform_values do |durations|
  median.call(durations)
end
observed_step_cost_s = observed_step_cost_lists.transform_values do |durations|
  median.call(durations)
end
step_cost_s = fallback_step_cost_s.merge(observed_step_cost_s)

web_job = jobs['web'] || {}
web_caches_trunk_tools = Array(web_job['steps']).any? do |step|
  step['name'].to_s.downcase.include?('cache trunk tools')
end
web_installs_system_wasm_bindgen = Array(web_job['steps']).any? do |step|
  step['name'].to_s.downcase.include?('install wasm-bindgen')
end
web_trunk_model_s = if web_caches_trunk_tools
  web_trunk_build_s.to_f
elsif web_installs_system_wasm_bindgen
  web_trunk_build_cold_system_wasm_bindgen_s.to_f
else
  web_trunk_build_cold_s.to_f
end

normalize_run = lambda do |run_text, working_directory|
  text = run_text.to_s.strip.gsub(/\s+/, ' ')
  wd = working_directory.to_s.strip
  return nil if text.empty?

  if text == './scripts/check-all.sh'
    return full_bundle
  end

  return web_preflight_s.to_f if text.include?('test:preflight')
  return web_trunk_model_s if wd.include?('apps/egui-web') && text == 'env -u NO_COLOR TRUNK_COLOR=never trunk build'
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
puts "METRIC web_trunk_build_s=#{web_trunk_build_s.to_f.round(3)}"
puts "METRIC web_trunk_build_cold_s=#{web_trunk_build_cold_s.to_f.round(3)}"
puts "METRIC web_trunk_build_cold_system_wasm_bindgen_s=#{web_trunk_build_cold_system_wasm_bindgen_s.to_f.round(3)}"
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
