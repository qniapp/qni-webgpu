const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

test('autoresearch script emits CI observation diagnostics for selection tier and view failures', async () => {
  const source = await fs.readFile(path.join(__dirname, '..', 'autoresearch.sh'), 'utf8')

  assert.match(source, /METRIC observed_ci_selection_tier=/)
  assert.match(source, /METRIC observed_ci_matching_run_count=/)
  assert.match(source, /METRIC observed_ci_exact_runtime_run_count=/)
  assert.match(source, /METRIC observed_ci_exact_run_runtime_count=/)
  assert.match(source, /METRIC observed_ci_view_failure_count=/)
})
