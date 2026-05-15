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
    indexLoadsGeneratedBootstrap: /href="\.trunk-generated\/bootstrap\.js"/.test(index),
    indexAvoidsCheckedInBootstrapTarget: !/data-target-path="bootstrap\.js"/.test(index),
    indexUsesModuleScript: /<script type="module" src="bootstrap\.js"><\/script>/.test(index),
    gitignoreIgnoresGeneratedBootstrap: /apps\/egui-web\/\.trunk-generated\//.test(gitignore),
  }, {
    hasBootstrapTs: true,
    hasBootstrapJs: false,
    buildScriptUsesTs: true,
    trunkHasHook: true,
    trunkPreBuild: true,
    trunkUsesPnpm: true,
    trunkBuildBootstrap: true,
    trunkWatchesBootstrapTs: true,
    indexLoadsGeneratedBootstrap: true,
    indexAvoidsCheckedInBootstrapTarget: true,
    indexUsesModuleScript: true,
    gitignoreIgnoresGeneratedBootstrap: true,
  })
})

export {}
