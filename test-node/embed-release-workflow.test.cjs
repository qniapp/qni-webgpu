const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { spawnSync } = require('node:child_process')

const workflowPath = path.join(__dirname, '..', '.github/workflows/embed-release.yml')
const workflow = () => fs.readFileSync(workflowPath, 'utf8')
const assets = ['qni-web.js', 'qni-web_bg.wasm', 'qni-embed.mjs', 'qni-embed-error.mjs', 'h.svg', 'design-system.css', 'h-gate.js', 'LICENSE', 'manifest.json', 'manifest.json.sha256']
const contracts = [
  ['only master push and manual triggers', /on:\n  push:\n    branches: \[master\]\n  workflow_dispatch:\n\npermissions:/],
  ['write permissions', /permissions:\n  contents: write\n  id-token: write\n  attestations: write/],
  ['full SHA concurrency without cancellation', /concurrency:\n  group: embed-release-\$\{\{ github.sha \}\}\n  cancel-in-progress: false/],
  ['full SHA tag', /TAG: embed-\$\{\{ github.sha \}\}/],
  ['guard immediately after credential-free checkout', /uses: actions\/checkout@v7\n\s+with:\n\s+persist-credentials: false\n\n      - name: Check tag and release absence/],
  ['master-only manual execution', /check:\n\s+if: github.ref == 'refs\/heads\/master'/],
  ['draft visibility uses a write-authorized isolated check', /check:\n\s+if: github.ref == 'refs\/heads\/master'\n\s+permissions:\n\s+contents: write\n\s+runs-on:/],
  ['build waits for the initial existence check', /build:\n\s+needs: check/],
  ['read-only build job', /build:[\s\S]*?permissions:\n\s+contents: read\n\s+runs-on:/],
  ['publishing waits for the isolated build', /publish:\n\s+needs: build/],
  ['hands verified files to the publishing job', /uses: actions\/upload-artifact@v7[\s\S]*?uses: actions\/download-artifact@v8[\s\S]*?- name: Verify staged release files/],
  ['pnpm 9', /version: '9'/],
  ['Node 22', /node-version: '22'/],
  ['wasm target', /rustup target add wasm32-unknown-unknown/],
  ['Rust cache', /uses: Swatinem\/rust-cache@v2/],
  ['pinned Trunk', /tool: trunk@0\.21\.14/],
  ['frozen dependencies', /pnpm install --frozen-lockfile\n\s+working-directory: apps\/web/],
  ['build module', /node apps\/web\/scripts\/embed-release.mjs build --output "\$STAGING" --source-commit "\$GITHUB_SHA"/],
  ['draft creation targets full SHA', /gh release create "\$TAG" --draft --target "\$GITHUB_SHA" --title .* --notes-file /],
  ['provenance for every staged file', /uses: actions\/attest-build-provenance@v3\n\s+with:\n\s+subject-path: \$\{\{ env.STAGING \}\}\/\*/],
  ['publishes as prerelease', /gh release edit "\$TAG" --draft=false --prerelease/],
  ['no destructive operations or pre-created tags', /^(?![\s\S]*(?:--clobber|gh release delete|gh api[^\n]*(?:DELETE|POST)|git (?:tag|push)|cleanup))[\s\S]*$/],
  ['notes source link', /https:\/\/github.com\/\$\{GITHUB_REPOSITORY\}\/commit\/\$\{GITHUB_SHA\}/],
  ['notes API version and manifest digest', /embed_api_version[\s\S]*manifest\.json\.sha256/],
]
for (const [name, pattern] of contracts) {
  test(`embed release ${name}`, () => assert.match(workflow(), pattern))
}

test('embed release orders guard, build, attestation, draft, upload, verification, publish', () => {
  const source = workflow()
  const markers = ['- name: Check tag and release absence', '- name: Build release', 'actions/attest-build-provenance@v3', 'gh release create', 'gh release upload', '- name: Verify uploaded assets', 'gh release edit']
  const positions = markers.map(marker => source.indexOf(marker))
  assert.equal(positions.every((position, index) => position >= 0 && (index === 0 || position > positions[index - 1])), true)
})

test('embed release declares the exact ordered asset list', () => {
  assert.deepEqual(workflow().match(/RELEASE_ASSETS: '([^']+)'/)[1].split(' '), assets)
})

test('CI embed build records source SHA and validates a dry-run without another job', () => {
  const ci = fs.readFileSync(path.join(__dirname, '..', '.github/workflows/ci.yml'), 'utf8')
  assert.match(ci, /- name: Web embed build\n\s+env:\n\s+QNI_SOURCE_COMMIT: \$\{\{ github.sha \}\}\n\s+run: pnpm run build:embed[\s\S]*?node apps\/web\/scripts\/embed-release.mjs dry-run --input apps\/web\/embed-dist --output "\$RUNNER_TEMP\/embed-release" --source-commit "\$GITHUB_SHA"\n\s+node apps\/web\/scripts\/embed-release.mjs verify --output "\$RUNNER_TEMP\/embed-release"[\s\S]*?- name: Upload Web Playwright bundle/)
})

class ReleaseMetadataFixture {
  static run(change) {
    const source = workflow().split('      - name: Verify uploaded assets\n')[1].split('\n      - name:')[0]
    const script = source.split("          node <<'NODE'\n")[1].split('\n          NODE')[0].split('\n').map(line => line.replace(/^          /, '')).join('\n')
    const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'embed-release-assets-'))
    try {
      for (const name of assets) fs.writeFileSync(path.join(directory, name), name)
      const release = { isDraft: true, assets: assets.map(name => ({ name, size: Buffer.byteLength(name) })) }
      change(release)
      fs.writeFileSync(path.join(directory, 'embed-release-assets.json'), JSON.stringify(release))
      return spawnSync(process.execPath, ['-e', script], { encoding: 'utf8', env: { ...process.env, STAGING: directory, RELEASE_ASSETS: assets.join(' '), RUNNER_TEMP: directory } })
    } finally {
      fs.rmSync(directory, { recursive: true, force: true })
    }
  }
}

test('uploaded asset validation accepts an exact draft asset set', () => {
  assert.equal(ReleaseMetadataFixture.run(() => {}).status, 0)
})

for (const [name, change] of [
  ['missing asset', release => { release.assets.pop() }],
  ['extra asset', release => { release.assets.push({ name: 'extra', size: 1 }) }],
  ['duplicate asset', release => { release.assets[1] = release.assets[0] }],
  ['wrong size', release => { release.assets[0].size += 1 }],
  ['published release', release => { release.isDraft = false }],
]) {
  test(`uploaded asset validation rejects ${name}`, () => {
    assert.notEqual(ReleaseMetadataFixture.run(change).status, 0)
  })
}

function runReleaseStep(scenario, stepName = 'Check tag and release absence') {
  const source = workflow()
  const block = source.split(`      - name: ${stepName}\n`)[1].split('\n      - name:')[0]
  const script = block.split('        run: |\n')[1].split('\n').filter(line => line.startsWith('          ') || line.trim() === '').map(line => line.replace(/^          /, '')).join('\n')
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'embed-release-guard-'))
  try {
    fs.writeFileSync(path.join(dir, 'gh'), `#!/usr/bin/env node
const args = process.argv.slice(2)
const scenario = process.env.SCENARIO
require('node:fs').appendFileSync(process.env.CALLS, JSON.stringify(args) + '\\n')
if (args[0] === 'release') {
  if (args[1] === 'create') { require('node:fs').writeFileSync(process.env.CREATED, 'yes'); process.exit(0) }
  if (args[1] === 'upload') process.exit(0)
  if (args[1] === 'edit') { require('node:fs').writeFileSync(process.env.PUBLISHED, 'yes'); process.exit(0) }
  process.exit(1)
}
if (args.includes('--paginate')) {
  if (scenario === 'list-error') process.exit(1)
  if (scenario === 'bad-json') { console.log('no'); process.exit(0) }
  const listCount = require('node:fs').readFileSync(process.env.CALLS, 'utf8').trim().split('\\n').map(JSON.parse).filter(call => call.includes('--paginate')).length
  const changed = listCount >= 3
  console.log(JSON.stringify([[], scenario === 'draft' || scenario === 'release' || scenario === 'publish-wrong' || require('node:fs').existsSync(process.env.CREATED) ? [{id: scenario === 'upload-id-change' && changed ? 43 : 42, tag_name: process.env.TAG, draft: scenario !== 'release' && !(scenario === 'upload-published' && changed)}] : []]))
  process.exit(0)
}
if (args.some(a => a.includes('/git/ref/')) && (scenario === 'tag' || require('node:fs').existsSync(process.env.PUBLISHED))) { console.log(JSON.stringify({object: {type: 'commit', sha: scenario === 'publish-wrong' ? 'b'.repeat(40) : process.env.GITHUB_SHA}})); process.exit(0) }
if (scenario === 'auth') { console.log('HTTP/2.0 401 Unauthorized'); process.exit(1) }
if (scenario === 'network') process.exit(1)
console.log('HTTP/2.0 404 Not Found\\n\\n{"message":"Not Found"}')
process.exit(1)
`, { mode: 0o755 })
    const result = spawnSync('bash', ['-euo', 'pipefail', '-c', script], { encoding: 'utf8', env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, SCENARIO: scenario, CALLS: path.join(dir, 'calls'), CREATED: path.join(dir, 'created'), PUBLISHED: path.join(dir, 'published'), RELEASE_ID: '42', TAG: `embed-${'a'.repeat(40)}`, GITHUB_SHA: 'a'.repeat(40), GITHUB_REPOSITORY: 'owner/repo', RUNNER_TEMP: dir, GITHUB_ENV: path.join(dir, 'env'), GH_TOKEN: 'mock', RELEASE_ASSETS: assets.join(' '), STAGING: dir } })
    return { ...result, calls: fs.readFileSync(path.join(dir, 'calls'), 'utf8').trim().split('\n').map(line => JSON.parse(line)) }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true })
  }
}
for (const scenario of ['tag', 'release', 'draft', 'auth', 'network', 'list-error', 'bad-json']) {
  test(`guard fails closed for ${scenario}`, () => assert.notEqual(runReleaseStep(scenario).status, 0))
}
test('guard permits confirmed tag and release absence', () => assert.equal(runReleaseStep('absent').status, 0))

for (const scenario of ['tag', 'release', 'draft', 'auth', 'network', 'list-error', 'bad-json']) {
  test(`draft creation makes no release writes when recheck finds ${scenario}`, () => {
    const result = runReleaseStep(scenario, 'Create draft and upload all assets')
    assert.equal(result.calls.some(args => args[0] === 'release'), false)
  })
  test(`draft creation fails when recheck finds ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Create draft and upload all assets').status, 0)
  })
}

for (const scenario of ['upload-id-change', 'upload-published']) {
  test(`upload stops when draft ownership changes after recording: ${scenario}`, () => {
    const result = runReleaseStep(scenario, 'Create draft and upload all assets')
    assert.deepEqual(result.calls.filter(args => args[0] === 'release').map(args => args[1]), ['create'])
  })
  test(`upload fails when draft ownership changes after recording: ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Create draft and upload all assets').status, 0)
  })
}

test('draft creation and upload succeed with confirmed ownership', () => {
  assert.equal(runReleaseStep('absent', 'Create draft and upload all assets').status, 0)
})

test('draft creation confirms ownership before uploading all ten assets', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets')
  assert.deepEqual(result.calls.map(args => args.slice(0, 2)), [['api', '--include'], ['api', '--include'], ['api', '--paginate'], ['release', 'create'], ['api', '--include'], ['api', '--paginate'], ['api', '--include'], ['api', '--paginate'], ['release', 'upload']])
})

test('draft upload includes exactly the ten declared assets', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets')
  assert.deepEqual(result.calls.at(-1).slice(3).map(filename => path.basename(filename)), assets)
})

for (const [name, pattern] of [
  ['creation', /node apps\/web\/scripts\/embed-release-guard.mjs assert-absent\n          gh release create/],
  ['upload', /node apps\/web\/scripts\/embed-release-guard.mjs assert-own-draft --release-id "\$release_id"\n          gh release upload/],
  ['publication', /node apps\/web\/scripts\/embed-release-guard.mjs assert-own-draft --release-id "\$RELEASE_ID"\n          gh release edit/],
  ['post-publication', /gh release edit[^\n]+\n          node apps\/web\/scripts\/embed-release-guard.mjs assert-published/],
  ['recorded identity', /release_id=\$\(node apps\/web\/scripts\/embed-release-guard.mjs record-draft\)\n          echo "RELEASE_ID=\$release_id" >> "\$GITHUB_ENV"/],
]) {
  test(`release guard orders ${name}`, () => assert.match(workflow(), pattern))
}

for (const scenario of ['tag', 'release', 'absent', 'auth', 'network', 'list-error', 'bad-json']) {
  test(`publication makes no release writes when recheck finds ${scenario}`, () => {
    const result = runReleaseStep(scenario, 'Publish prerelease')
    assert.equal(result.calls.some(args => args[0] === 'release'), false)
  })
  test(`publication fails when recheck finds ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Publish prerelease').status, 0)
  })
}

test('publication succeeds with a matching published tag', () => {
  assert.equal(runReleaseStep('draft', 'Publish prerelease').status, 0)
})

test('publication verifies the created tag after the edit', () => {
  const result = runReleaseStep('draft', 'Publish prerelease')
  assert.deepEqual(result.calls.map(args => args.slice(0, 2)), [['api', '--include'], ['api', '--paginate'], ['release', 'edit'], ['api', '--include']])
})

test('publication fails for a wrong tag commit', () => {
  assert.notEqual(runReleaseStep('publish-wrong', 'Publish prerelease').status, 0)
})

test('publication reports a wrong tag commit', () => {
  assert.match(runReleaseStep('publish-wrong', 'Publish prerelease').stderr, /matches GITHUB_SHA: false/)
})

test('publication makes no further writes for a wrong tag commit', () => {
  const result = runReleaseStep('publish-wrong', 'Publish prerelease')
  assert.deepEqual(result.calls.filter(args => args[0] === 'release').map(args => args[1]), ['edit'])
})

test('every release write has its corresponding guard on the immediately preceding line', () => {
  const lines = workflow().split('\n').map(line => line.trim())
  const guards = { create: 'assert-absent', upload: 'assert-own-draft --release-id "$release_id"', edit: 'assert-own-draft --release-id "$RELEASE_ID"' }
  const writes = lines.flatMap((line, index) => {
    const command = line.match(/^gh release (create|upload|edit) /)
    return command ? [[command[1], lines[index - 1]]] : []
  })
  assert.deepEqual(writes, Object.entries(guards).map(([command, guard]) => [command, `node apps/web/scripts/embed-release-guard.mjs ${guard}`]))
})

test('write tokens are scoped only to the four gh steps', () => {
  const lines = workflow().split('\n').filter(line => line.includes('GH_TOKEN:'))
  assert.deepEqual(lines, Array(4).fill('          GH_TOKEN: ${{ github.token }}'))
})
