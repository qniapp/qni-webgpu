const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')

test('ABCI follow-up docs capture non-environment decisions', async () => {
  const apiDoc = await fs.readFile(path.join(rootDir, 'docs', 'implementation', 'external-gpu-api-compatibility.md'), 'utf8')
  const deployDoc = await fs.readFile(path.join(rootDir, 'docs', 'implementation', 'abci-deployment-guide.md'), 'utf8')
  const migrationDoc = await fs.readFile(path.join(rootDir, 'docs', 'implementation', 'qni-gl-migration-notes.md'), 'utf8')
  const todo = await fs.readFile(path.join(rootDir, 'docs', 'implementation', 'abci-support-todo.html'), 'utf8')

  assert.deepEqual({
    measurementContractRejectsMeasuredBits: /measuredBits` は採用しない/.test(apiDoc),
    qasmDecisionIsExplicit: /QASM3 書き出しは、ABCI 初期対応では採用しない/.test(apiDoc),
    cacheDecisionIsExplicit: /CachedQiskitRunner` 相当は、ABCI 初期対応では採用しない/.test(apiDoc),
    deploymentGuideReferencesSmokeScript: /scripts\/smoke-abci-container\.sh/.test(deployDoc),
    migrationNotesCompareBackendJsonAndRun: /`\/backend\.json` は使わない/.test(migrationDoc),
    todoMarksGpuUnverifiedWork: /実装済み・GPU 未検証/.test(todo),
  }, {
    measurementContractRejectsMeasuredBits: true,
    qasmDecisionIsExplicit: true,
    cacheDecisionIsExplicit: true,
    deploymentGuideReferencesSmokeScript: true,
    migrationNotesCompareBackendJsonAndRun: true,
    todoMarksGpuUnverifiedWork: true,
  })
})

test('ABCI smoke script verifies production runner constraints without enabling mock fallback', async () => {
  const smokeScript = await fs.readFile(path.join(rootDir, 'scripts', 'smoke-abci-container.sh'), 'utf8')

  assert.deepEqual({
    runsContainerWithGpuArgs: /DOCKER_GPU_ARGS/.test(smokeScript),
    validatesSkipFlag: /validate_bool QNI_SMOKE_RUN_GPU/.test(smokeScript),
    honorsEmptyGpuArgsOverride: /QNI_SMOKE_DOCKER_GPU_ARGS\+x/.test(smokeScript),
    checksHealthRunnerSet: /defaultRunner'\) != 'qiskit-gpu'/.test(smokeScript),
    requiresBasicAuth: /QNI_REQUIRE_BASIC_AUTH=true/.test(smokeScript),
    assertsNoAuthRejected: /expected unauthenticated \/run to return 401/.test(smokeScript),
    assertsMockRejected: /expected production container to reject mock runner/.test(smokeScript),
    runsGpuWhenEnabled: /RUN_GPU_CHECK/.test(smokeScript) && /runner'\) != 'qiskit-gpu'/.test(smokeScript),
    doesNotSetCpuDevRunner: !/qiskit-cpu-dev/.test(smokeScript),
  }, {
    runsContainerWithGpuArgs: true,
    validatesSkipFlag: true,
    honorsEmptyGpuArgsOverride: true,
    checksHealthRunnerSet: true,
    requiresBasicAuth: true,
    assertsNoAuthRejected: true,
    assertsMockRejected: true,
    runsGpuWhenEnabled: true,
    doesNotSetCpuDevRunner: true,
  })
})
