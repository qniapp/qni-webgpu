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

measure_median_seconds() {
  local label="$1"
  local repeats="$2"
  shift 2

  python - "$label" "$repeats" "$@" <<'PY'
import statistics
import subprocess
import sys
import time
from pathlib import Path

label = sys.argv[1]
repeats = int(sys.argv[2])
cmd = sys.argv[3:]
max_attempts = repeats + 2
results = []
last_failure = None
attempt = 0

while len(results) < repeats and attempt < max_attempts:
    attempt += 1
    start = time.perf_counter_ns()
    completed = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False)
    end = time.perf_counter_ns()
    Path(f"/tmp/{label}-attempt-{attempt}.log").write_text(completed.stdout)

    if completed.returncode == 0:
        results.append((end - start) / 1_000_000_000)
    else:
        last_failure = completed

if len(results) < repeats:
    if last_failure is not None:
        raise subprocess.CalledProcessError(last_failure.returncode, cmd, output=last_failure.stdout)
    raise RuntimeError(f"{label}: unable to collect {repeats} successful samples")

print(f"{statistics.median(results):.3f}")
PY
}

WEB_PREFLIGHT_S=$(measure_median_seconds web-preflight 3 pnpm -C "$ROOT_DIR/apps/egui-web" run test:preflight)
WEB_TRUNK_BUILD_S=$(measure_median_seconds web-trunk-build 3 bash -lc "cd '$ROOT_DIR/apps/egui-web' && env -u NO_COLOR TRUNK_COLOR=never trunk build")
WEB_TRUNK_BUILD_COLD_S=$(measure_median_seconds web-trunk-build-cold 3 bash -lc "cd '$ROOT_DIR/apps/egui-web' && tmp_cache=\$(mktemp -d) && trap 'rm -rf \"\$tmp_cache\"' EXIT && XDG_CACHE_HOME=\$tmp_cache env -u NO_COLOR TRUNK_COLOR=never trunk build")
WEB_TRUNK_BUILD_COLD_SYSTEM_WASM_BINDGEN_S=$(measure_median_seconds web-trunk-build-cold-system-wasm-bindgen 3 bash -lc "cd '$ROOT_DIR/apps/egui-web' && tmp_cache=\$(mktemp -d) && candidate_dir=\$(python - <<'PY'
from pathlib import Path
matches = sorted(Path.home().glob('.cache/trunk/wasm-bindgen-*/wasm-bindgen'))
print(matches[-1].parent if matches else '')
PY
) && trap 'rm -rf \"\$tmp_cache\"' EXIT && if [ -n \"\$candidate_dir\" ]; then PATH=\"\$candidate_dir:\$PATH\" XDG_CACHE_HOME=\$tmp_cache env -u NO_COLOR TRUNK_COLOR=never trunk build; else XDG_CACHE_HOME=\$tmp_cache env -u NO_COLOR TRUNK_COLOR=never trunk build; fi")
WEB_BDD_S=$(measure_median_seconds web-bdd 3 bash -lc "cd '$ROOT_DIR/apps/egui-web'
set -euo pipefail
port=\$(python - <<'PY'
import socket
sock = socket.socket()
sock.bind(('127.0.0.1', 0))
print(sock.getsockname()[1])
sock.close()
PY
)
python3 -m http.server \"\$port\" --bind 127.0.0.1 --directory dist >/tmp/egui-web-bdd-benchmark.log 2>&1 &
server_pid=\$!
trap 'kill \"\$server_pid\" 2>/dev/null || true' EXIT
QNI_EGUI_WEB_PORT=\$port python3 - <<'PY'
import os
import time
import urllib.request

url = 'http://127.0.0.1:{}/'.format(os.environ['QNI_EGUI_WEB_PORT'])
deadline = time.time() + 20
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url) as response:
            response.read()
        break
    except Exception:
        time.sleep(0.25)
else:
    raise SystemExit('Timed out waiting for static egui-web server')
PY
CI=1 QNI_EGUI_WEB_EXTERNAL_SERVER=1 QNI_EGUI_WEB_BASE_URL=http://127.0.0.1:\$port pnpm run test:bdd")
WEB_LEGACY_S=$(measure_median_seconds web-legacy 3 bash -lc "cd '$ROOT_DIR/apps/egui-web'
set -euo pipefail
port=\$(python - <<'PY'
import socket
sock = socket.socket()
sock.bind(('127.0.0.1', 0))
print(sock.getsockname()[1])
sock.close()
PY
)
python3 -m http.server \"\$port\" --bind 127.0.0.1 --directory dist >/tmp/egui-web-legacy-benchmark.log 2>&1 &
server_pid=\$!
trap 'kill \"\$server_pid\" 2>/dev/null || true' EXIT
QNI_EGUI_WEB_PORT=\$port python3 - <<'PY'
import os
import time
import urllib.request

url = 'http://127.0.0.1:{}/'.format(os.environ['QNI_EGUI_WEB_PORT'])
deadline = time.time() + 20
while time.time() < deadline:
    try:
        with urllib.request.urlopen(url) as response:
            response.read()
        break
    except Exception:
        time.sleep(0.25)
else:
    raise SystemExit('Timed out waiting for static egui-web server')
PY
CI=1 QNI_EGUI_WEB_EXTERNAL_SERVER=1 QNI_EGUI_WEB_BASE_URL=http://127.0.0.1:\$port pnpm run test:pw-legacy")
MCP_CHECK_S=$(measure_median_seconds mcp-check 3 pnpm -C "$ROOT_DIR/apps/mcp-qni" check)
TUI_CHECK_S=$(measure_median_seconds tui-check 3 make -C "$ROOT_DIR" check)

WEB_LOCAL_S=$(python - "$WEB_PREFLIGHT_S" "$WEB_BDD_S" "$WEB_LEGACY_S" <<'PY'
import sys
print(f"{sum(float(v) for v in sys.argv[1:]):.3f}")
PY
)

ruby - "$ROOT_DIR" "$WORKFLOW_PATH" "$WEB_PREFLIGHT_S" "$WEB_TRUNK_BUILD_S" "$WEB_TRUNK_BUILD_COLD_S" "$WEB_TRUNK_BUILD_COLD_SYSTEM_WASM_BINDGEN_S" "$WEB_BDD_S" "$WEB_LEGACY_S" "$MCP_CHECK_S" "$TUI_CHECK_S" <<'RUBY'
require 'json'
require 'open3'
require 'set'
require 'time'
require 'yaml'

root_dir, workflow_path, web_preflight_s, web_trunk_build_s, web_trunk_build_cold_s, web_trunk_build_cold_system_wasm_bindgen_s, web_bdd_s, web_legacy_s, mcp_check_s, tui_check_s = ARGV
config = YAML.load_file(workflow_path)
jobs = config.fetch('jobs')
workflow_match_pathspecs = [
  '.github/workflows/ci.yml',
]
run_runtime_match_pathspecs = [
  'Makefile',
  'scripts/check-all.sh',
  'apps/egui-web',
  'apps/mcp-qni',
  'apps/tui',
]
exact_runtime_match_pathspecs = workflow_match_pathspecs + run_runtime_match_pathspecs

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
exact_run_runtime_job_step_cost_s = {}
exact_run_runtime_job_step_cost_lists = Hash.new { |hash, key| hash[key] = [] }
exact_run_runtime_step_cost_s = {}
exact_run_runtime_step_cost_lists = Hash.new { |hash, key| hash[key] = [] }
observed_job_overhead_s = {}
observed_job_overhead_lists = Hash.new { |hash, key| hash[key] = [] }
exact_run_runtime_job_overhead_s = {}
exact_run_runtime_job_overhead_lists = Hash.new { |hash, key| hash[key] = [] }
exact_run_runtime_job_names = Set.new
exact_run_runtime_step_job_counts = Hash.new(0)
observed_run_id = nil
observed_run_count = 0
observed_selection_tier = 0
observed_matching_run_count = 0
observed_exact_runtime_run_count = 0
observed_exact_run_runtime_count = 0
observed_view_failure_count = 0
selected_records_exact_runtime = false
can_use_exact_run_runtime_costs = false

expected_job_steps = jobs.transform_values do |job|
  Array(job['steps']).map { |step| step['name'].to_s }
end
current_step_job_counts = expected_job_steps.values.flatten.tally

if !branch_name.to_s.empty?
  run_list_out, run_list_status = Open3.capture2(
    'gh', 'run', 'list',
    '--branch', branch_name,
    '--workflow', 'CI',
    '--limit', '20',
    '--json', 'databaseId,conclusion,headSha,attempt'
  )

  if run_list_status.success?
    successful_runs = JSON.parse(run_list_out).select { |run| run['conclusion'] == 'success' }
    run_records = []
    view_failures = 0
    tracked_exact_runtime_tree_clean = Open3.capture2(
      'git', '-C', root_dir, 'diff', '--quiet', 'HEAD', '--', *exact_runtime_match_pathspecs
    )[1].success?
    untracked_exact_runtime_files_out, = Open3.capture2(
      'git', '-C', root_dir, 'ls-files', '--others', '--exclude-standard', '--', *exact_runtime_match_pathspecs
    )
    can_exact_match_runtime_code = tracked_exact_runtime_tree_clean && untracked_exact_runtime_files_out.strip.empty?
    tracked_run_runtime_tree_clean = Open3.capture2(
      'git', '-C', root_dir, 'diff', '--quiet', 'HEAD', '--', *run_runtime_match_pathspecs
    )[1].success?
    untracked_run_runtime_files_out, = Open3.capture2(
      'git', '-C', root_dir, 'ls-files', '--others', '--exclude-standard', '--', *run_runtime_match_pathspecs
    )
    can_use_exact_run_runtime_costs = tracked_run_runtime_tree_clean && untracked_run_runtime_files_out.strip.empty?

    successful_runs.each do |run|
      matches_current_runtime_code = false
      if can_exact_match_runtime_code
        matches_current_runtime_code = Open3.capture2(
          'git', '-C', root_dir, 'diff', '--quiet', run.fetch('headSha').to_s, 'HEAD', '--', *exact_runtime_match_pathspecs
        )[1].success?
      end
      matches_current_run_runtime_code = false
      if can_use_exact_run_runtime_costs
        matches_current_run_runtime_code = Open3.capture2(
          'git', '-C', root_dir, 'diff', '--quiet', run.fetch('headSha').to_s, 'HEAD', '--', *run_runtime_match_pathspecs
        )[1].success?
      end

      1.upto(run.fetch('attempt', 1).to_i) do |attempt_number|
        run_view_out, run_view_status = Open3.capture2(
          'gh', 'run', 'view', run.fetch('databaseId').to_s,
          '--attempt', attempt_number.to_s,
          '--json', 'jobs'
        )
        unless run_view_status.success?
          view_failures += 1
          next
        end

        run_jobs = JSON.parse(run_view_out).fetch('jobs')
        attempt_success = run_jobs.all? { |job| job['conclusion'].to_s == 'success' }
        next unless attempt_success

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
          attempt: attempt_number,
          jobs: run_jobs,
          matching: matches_current_workflow,
          exact_runtime: matches_current_workflow && matches_current_runtime_code,
          exact_run_runtime: matches_current_run_runtime_code,
        }
      end
    end

    exact_runtime_records = run_records.select { |record| record[:exact_runtime] }
    matching_records = run_records.select { |record| record[:matching] }
    selected_records = if exact_runtime_records.any?
      exact_runtime_records.first(5)
    elsif matching_records.any?
      matching_records.first(5)
    else
      run_records.first(5)
    end
    observed_selection_tier = if exact_runtime_records.any?
      2
    elsif matching_records.any?
      1
    else
      0
    end
    observed_matching_run_count = matching_records.size
    observed_exact_runtime_run_count = exact_runtime_records.size
    observed_exact_run_runtime_count = run_records.count { |record| record[:exact_run_runtime] }
    observed_view_failure_count = view_failures
    selected_records_exact_runtime = exact_runtime_records.any?
    observed_run_id = selected_records.first&.fetch(:id, nil)
    observed_run_count = selected_records.size

    selected_records.each do |record|
      record.fetch(:jobs).each do |job|
        counted_duration_s = 0.0

        Array(job['steps']).each do |step|
          next unless step['status'] == 'completed'
          next if step['name'].to_s.start_with?('Post ')
          next if step['name'].to_s == 'Complete job'

          started_at = step['startedAt']
          completed_at = step['completedAt']
          next if started_at.to_s.empty? || completed_at.to_s.empty?

          duration_s = Time.iso8601(completed_at) - Time.iso8601(started_at)
          counted_duration_s += duration_s
          observed_job_step_cost_lists[[job['name'], step['name']]] << duration_s
          observed_step_cost_lists[step['name']] << duration_s
        end

        job_started_at = job['startedAt']
        job_completed_at = job['completedAt']
        next if job_started_at.to_s.empty? || job_completed_at.to_s.empty?

        job_wall_s = Time.iso8601(job_completed_at) - Time.iso8601(job_started_at)
        observed_job_overhead_lists[job['name']] << [job_wall_s - counted_duration_s, 0.0].max
      end
    end

    run_records.select { |record| record[:exact_run_runtime] }.first(5).each do |record|
      record_step_job_counts = Hash.new(0)

      record.fetch(:jobs).each do |job|
        exact_run_runtime_job_names << job['name'].to_s
        counted_duration_s = 0.0

        Array(job['steps']).each do |step|
          next unless step['status'] == 'completed'
          next if step['name'].to_s.start_with?('Post ')
          next if step['name'].to_s == 'Complete job'

          started_at = step['startedAt']
          completed_at = step['completedAt']
          next if started_at.to_s.empty? || completed_at.to_s.empty?

          duration_s = Time.iso8601(completed_at) - Time.iso8601(started_at)
          counted_duration_s += duration_s
          exact_run_runtime_job_step_cost_lists[[job['name'], step['name']]] << duration_s
          exact_run_runtime_step_cost_lists[step['name']] << duration_s
          record_step_job_counts[step['name']] += 1
        end

        job_started_at = job['startedAt']
        job_completed_at = job['completedAt']
        unless job_started_at.to_s.empty? || job_completed_at.to_s.empty?
          job_wall_s = Time.iso8601(job_completed_at) - Time.iso8601(job_started_at)
          exact_run_runtime_job_overhead_lists[job['name']] << [job_wall_s - counted_duration_s, 0.0].max
        end
      end

      record_step_job_counts.each do |step_name, count|
        exact_run_runtime_step_job_counts[step_name] = [exact_run_runtime_step_job_counts[step_name], count].max
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
exact_run_runtime_job_step_cost_s = exact_run_runtime_job_step_cost_lists.transform_values do |durations|
  median.call(durations)
end
exact_run_runtime_step_cost_s = exact_run_runtime_step_cost_lists.transform_values do |durations|
  median.call(durations)
end
observed_job_overhead_s = observed_job_overhead_lists.transform_values do |durations|
  median.call(durations)
end
exact_run_runtime_job_overhead_s = exact_run_runtime_job_overhead_lists.transform_values do |durations|
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

compile_heavy_run_step = lambda do |step_name, text|
  step_name == 'Web trunk build' ||
    text.start_with?('cargo ') ||
    text == 'make check' ||
    (text.include?('make -C') && text.include?('check'))
end

normalize_run = lambda do |job_name, step_name, run_text, working_directory|
  text = run_text.to_s.strip.gsub(/\s+/, ' ')
  wd = working_directory.to_s.strip
  return nil if text.empty?

  if can_use_exact_run_runtime_costs
    exact_job_step_cost = exact_run_runtime_job_step_cost_s[[job_name, step_name]]
    return exact_job_step_cost if exact_job_step_cost

    if exact_run_runtime_job_names.include?(job_name)
      exact_cost = exact_run_runtime_step_cost_s[step_name]
      return exact_cost if exact_cost
    end

    if !exact_run_runtime_job_names.include?(job_name) && compile_heavy_run_step.call(step_name, text)
      renamed_compile_heavy_cost = fallback_step_cost_s[step_name] || step_cost_s[step_name]
      return renamed_compile_heavy_cost if renamed_compile_heavy_cost
    end

    if current_step_job_counts.fetch(step_name, 0) > exact_run_runtime_step_job_counts.fetch(step_name, 0) &&
       compile_heavy_run_step.call(step_name, text)
      conservative_cost = fallback_step_cost_s[step_name] || step_cost_s[step_name]
      return conservative_cost if conservative_cost
    end
  end

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

    if selected_records_exact_runtime && observed_job_step_cost_s.key?([job_name, name])
      total_s += observed_job_step_cost_s.fetch([job_name, name])
      next
    end

    run_cost = normalize_run.call(job_name, name, step['run'], step['working-directory'])
    workflow_only_observed_cost = nil
    if observed_selection_tier == 1 && observed_job_step_cost_s.key?([job_name, name])
      workflow_only_observed_cost = observed_job_step_cost_s.fetch([job_name, name])
    end
    fallback_observed_step_cost = nil
    if observed_selection_tier == 0 && run_cost && observed_step_cost_s.key?(name)
      fallback_observed_step_cost = observed_step_cost_s.fetch(name)
    end

    if workflow_only_observed_cost && run_cost
      total_s += [workflow_only_observed_cost, run_cost].max
      next
    end

    if fallback_observed_step_cost
      total_s += [fallback_observed_step_cost, run_cost].max
      next
    end

    if workflow_only_observed_cost
      total_s += workflow_only_observed_cost
      next
    end

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
  if selected_records_exact_runtime
    total_s += observed_job_overhead_s.fetch(job_name, 0.0)
  elsif can_use_exact_run_runtime_costs
    total_s += exact_run_runtime_job_overhead_s.fetch(job_name, 0.0)
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
puts "METRIC web_static_suites_s=0"
puts "METRIC mcp_local_s=#{mcp_check_s.to_f.round(3)}"
puts "METRIC tui_local_s=#{tui_check_s.to_f.round(3)}"
puts "METRIC modeled_jobs=#{job_totals.size}"
puts "METRIC modeled_unknown_steps=#{unknown_steps.size}"
puts "METRIC observed_ci_run_id=#{observed_run_id || 0}"
puts "METRIC observed_ci_run_count=#{observed_run_count}"
puts "METRIC observed_ci_selection_tier=#{observed_selection_tier}"
puts "METRIC observed_ci_matching_run_count=#{observed_matching_run_count}"
puts "METRIC observed_ci_exact_runtime_run_count=#{observed_exact_runtime_run_count}"
puts "METRIC observed_ci_exact_run_runtime_count=#{observed_exact_run_runtime_count}"
puts "METRIC observed_ci_view_failure_count=#{observed_view_failure_count}"
unknown_steps.each do |job_name, step_name, run_text|
  warn "UNMODELED_STEP #{job_name} :: #{step_name} :: #{run_text}"
end
RUBY
