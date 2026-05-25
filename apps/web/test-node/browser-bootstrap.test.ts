const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const repoRoot = path.join(rootDir, '..', '..')

test('browser bootstrap source is TypeScript without a checked-in JavaScript wrapper', async () => {
  const accessOk = async (filePath: string): Promise<boolean> => fs.access(filePath).then(() => true, () => false)
  const pkg = JSON.parse(await fs.readFile(path.join(rootDir, 'package.json'), 'utf8'))
  const trunkConfig = await fs.readFile(path.join(rootDir, 'Trunk.toml'), 'utf8')
  const index = await fs.readFile(path.join(rootDir, 'index.html'), 'utf8')
  const gitignore = await fs.readFile(path.join(repoRoot, '.gitignore'), 'utf8')

  assert.deepEqual({
    hasBootstrapTs: await accessOk(path.join(rootDir, 'bootstrap.ts')),
    hasBootstrapJs: await accessOk(path.join(rootDir, 'bootstrap.js')),
    buildScriptUsesTs: /tsc bootstrap\.ts/.test(pkg.scripts['build:bootstrap']),
    trunkHasHook: /\[\[hooks\]\]/.test(trunkConfig),
    trunkPreBuild: /stage = "pre_build"/.test(trunkConfig),
    trunkUsesPnpm: /command = "pnpm"/.test(trunkConfig),
    trunkBuildBootstrap: /"build:bootstrap"/.test(trunkConfig),
    trunkWatchesBootstrapTs: /watch = \["bootstrap\.ts"\]/.test(trunkConfig),
    bootstrapImportsWasmNextToBootstrapScript: /new URL\('qni-web\.js', bootstrapScriptUrl\(\)\)/.test(
      await fs.readFile(path.join(rootDir, 'bootstrap.ts'), 'utf8'),
    ),
    indexLoadsGeneratedBootstrap: /href="\.trunk-generated\/bootstrap\.js"/.test(index),
    indexAvoidsCheckedInBootstrapTarget: !/data-target-path="bootstrap\.js"/.test(index),
    indexUsesModuleScript: /<script type="module" src="bootstrap\.js"><\/script>/.test(index),
    gitignoreIgnoresGeneratedBootstrap: /apps\/web\/\.trunk-generated\//.test(gitignore),
  }, {
    hasBootstrapTs: true,
    hasBootstrapJs: false,
    buildScriptUsesTs: true,
    trunkHasHook: true,
    trunkPreBuild: true,
    trunkUsesPnpm: true,
    trunkBuildBootstrap: true,
    trunkWatchesBootstrapTs: true,
    bootstrapImportsWasmNextToBootstrapScript: true,
    indexLoadsGeneratedBootstrap: true,
    indexAvoidsCheckedInBootstrapTarget: true,
    indexUsesModuleScript: true,
    gitignoreIgnoresGeneratedBootstrap: true,
  })
})

test('browser bootstrap defaults Qiskit backend to same-origin outside local trunk dev', async () => {
  const source = await fs.readFile(path.join(rootDir, 'bootstrap.ts'), 'utf8')

  assert.deepEqual({
    keepsLocalTrunkBackend: /window\.location\.port === '4174'/.test(source),
    usesSameOriginRunPath: /new URL\('run', window\.location\.href\)/.test(source),
    fetchesThroughUrlHelper: /fetch\(qiskitBackendUrl\(\)/.test(source),
  }, {
    keepsLocalTrunkBackend: true,
    usesSameOriginRunPath: true,
    fetchesThroughUrlHelper: true,
  })
})

export {}
