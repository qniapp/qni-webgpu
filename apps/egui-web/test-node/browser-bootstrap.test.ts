const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const repoRoot = path.join(rootDir, '..', '..')

test('browser bootstrap source is TypeScript without a checked-in JavaScript wrapper', async () => {
  await assert.doesNotReject(() => fs.access(path.join(rootDir, 'bootstrap.ts')))
  await assert.rejects(() => fs.access(path.join(rootDir, 'bootstrap.js')), /ENOENT/)

  const pkg = JSON.parse(await fs.readFile(path.join(rootDir, 'package.json'), 'utf8'))
  assert.match(pkg.scripts['build:bootstrap'], /tsc bootstrap\.ts/)

  const trunkConfig = await fs.readFile(path.join(rootDir, 'Trunk.toml'), 'utf8')
  assert.match(trunkConfig, /\[\[hooks\]\]/)
  assert.match(trunkConfig, /stage = "pre_build"/)
  assert.match(trunkConfig, /command = "pnpm"/)
  assert.match(trunkConfig, /"build:bootstrap"/)
  assert.match(trunkConfig, /watch = \["bootstrap\.ts"\]/)

  const index = await fs.readFile(path.join(rootDir, 'index.html'), 'utf8')
  assert.match(index, /href="\.trunk-generated\/bootstrap\.js"/)
  assert.doesNotMatch(index, /data-target-path="bootstrap\.js"/)
  assert.match(index, /<script type="module" src="bootstrap\.js"><\/script>/)

  const gitignore = await fs.readFile(path.join(repoRoot, '.gitignore'), 'utf8')
  assert.match(gitignore, /apps\/egui-web\/\.trunk-generated\//)
})

export {}
