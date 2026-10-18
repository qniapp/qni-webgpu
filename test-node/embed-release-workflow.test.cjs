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
  ['draft creation targets full SHA', /gh api --method POST .*\/releases[\s\S]*?-f target_commitish="\$GITHUB_SHA"/],
  ['provenance for every staged file', /uses: actions\/attest-build-provenance@v3\n\s+with:\n\s+subject-path: \$\{\{ env.STAGING \}\}\/\*/],
  ['publishes as prerelease', /gh api --method PATCH .*\/releases\/\$RELEASE_ID" -F draft=false -F prerelease=true/],
  ['no destructive operations or pre-created tags', /^(?![\s\S]*(?:--clobber|gh release delete|gh api[^\n]*DELETE|git (?:tag|push)|cleanup))[\s\S]*$/],
  ['notes source link', /https:\/\/github.com\/\$\{GITHUB_REPOSITORY\}\/commit\/\$\{GITHUB_SHA\}/],
  ['notes API version and manifest digest', /embed_api_version[\s\S]*manifest\.json\.sha256/],
]
for (const [name, pattern] of contracts) {
  test(`embed release ${name}`, () => assert.match(workflow(), pattern))
}

test('embed release orders guard, build, attestation, draft, upload, verification, publish', () => {
  const source = workflow()
  const markers = ['- name: Check tag and release absence', '- name: Build release', 'actions/attest-build-provenance@v3', 'gh api --method POST', 'https://uploads.github.com/', '- name: Verify uploaded assets', 'gh api --method PATCH']
  const positions = markers.map(marker => source.indexOf(marker))
  assert.equal(positions.every((position, index) => position >= 0 && (index === 0 || position > positions[index - 1])), true)
})

test('embed release never rediscovers or mutates a release by tag', () => {
  assert.doesNotMatch(workflow(), /record-draft|gh release (?:create|upload|edit|view)\b/)
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
      const release = { draft: true, assets: assets.map(name => ({ name, size: Buffer.byteLength(name) })) }
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
  ['published release', release => { release.draft = false }],
]) {
  test(`uploaded asset validation rejects ${name}`, () => {
    assert.notEqual(ReleaseMetadataFixture.run(change).status, 0)
  })
}

const payloadTypes = ['text/javascript; charset=utf-8', 'application/wasm', 'text/javascript; charset=utf-8', 'text/javascript; charset=utf-8', 'image/svg+xml', 'text/css; charset=utf-8', 'text/javascript; charset=utf-8', 'text/plain; charset=utf-8']
const uploadTypes = [...payloadTypes, 'application/json', 'text/plain; charset=utf-8']
const stagedManifest = () => ({ files: assets.slice(0, -2).map((name, index) => ({ name, content_type: payloadTypes[index] })) })

function runReleaseStep(scenario, stepName = 'Check tag and release absence', changeManifest = () => {}) {
  const source = workflow()
  const block = source.split(`      - name: ${stepName}\n`)[1].split('\n      - name:')[0]
  const script = block.split('        run: |\n')[1].split('\n').filter(line => line.startsWith('          ') || line.trim() === '').map(line => line.replace(/^          /, '')).join('\n')
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'embed-release-guard-'))
  try {
    fs.writeFileSync(path.join(dir, 'gh'), `#!/usr/bin/env node
const args = process.argv.slice(2)
const scenario = process.env.SCENARIO
require('node:fs').appendFileSync(process.env.CALLS, JSON.stringify({ args, token: process.env.GH_TOKEN, ...(args.includes('--input') ? { size: require('node:fs').statSync(args[args.indexOf('--input') + 1]).size } : {}) }) + '\\n')
const fs = require('node:fs')
const endpoint = args.find(a => a.startsWith('repos/') || a.startsWith('https://')) || ''
const method = args[args.indexOf('--method') + 1]
const release = {id: 42, tag_name: process.env.TAG, target_commitish: process.env.GITHUB_SHA, draft: true, assets: JSON.parse(process.env.ASSET_METADATA)}
if (method === 'POST' && endpoint.endsWith('/releases')) {
  fs.writeFileSync(process.env.CREATED, 'yes')
  const changes = { 'id-zero': {id: 0}, 'id-negative': {id: -1}, 'id-string': {id: '42'}, 'id-fraction': {id: 1.5}, 'id-unsafe': {id: 9007199254740992}, 'id-missing': {id: undefined}, 'created-published': {draft: false}, 'created-tag': {tag_name: 'wrong'}, 'created-sha': {target_commitish: 'b'.repeat(40)} }
  console.log(scenario === 'created-json' ? 'invalid' : JSON.stringify({...release, ...changes[scenario]}))
  process.exit(0)
}
if (method === 'POST' && endpoint.startsWith('https://uploads.github.com/')) process.exit(0)
if (method === 'PATCH') { fs.writeFileSync(process.env.PUBLISHED, 'yes'); process.exit(0) }
if (endpoint.endsWith('/releases/42')) {
  const calls = fs.readFileSync(process.env.CALLS, 'utf8').trim().split('\\n').map(JSON.parse)
  const uploaded = calls.filter(c => c.args.some(a => a.startsWith('https://uploads.github.com/'))).length
  if (scenario === 'absent' && !fs.existsSync(process.env.CREATED)) { console.log('HTTP/2.0 404 Not Found\\n\\n{}'); process.exit(1) }
  if (scenario === 'upload-id-change') release.id = 43
  if (scenario === 'upload-published' || (scenario === 'midway-published' && uploaded === 4) || scenario === 'release') release.draft = false
  if (scenario === 'upload-sha-change') release.target_commitish = 'b'.repeat(40)
  if (scenario === 'upload-tag-change') release.tag_name = 'wrong'
  if (scenario !== 'auth' && scenario !== 'network' && scenario !== 'list-error' && scenario !== 'bad-json') { console.log(JSON.stringify(release)); process.exit(0) }
}
if (args.includes('--paginate')) {
  if (scenario === 'list-error') process.exit(1)
  if (scenario === 'bad-json') { console.log('no'); process.exit(0) }
  console.log(JSON.stringify([[], scenario === 'draft' || scenario === 'release' || scenario === 'publish-wrong' ? [{id: 42, tag_name: process.env.TAG, draft: scenario !== 'release'}] : []]))
  process.exit(0)
}
if (args.some(a => a.includes('/git/ref/')) && (scenario === 'tag' || (scenario === 'upload-tag-created' && require('node:fs').existsSync(process.env.CREATED)) || require('node:fs').existsSync(process.env.PUBLISHED))) { console.log(JSON.stringify({object: {type: 'commit', sha: scenario === 'publish-wrong' ? 'b'.repeat(40) : process.env.GITHUB_SHA}})); process.exit(0) }
if (scenario === 'auth') { console.log('HTTP/2.0 401 Unauthorized'); process.exit(1) }
if (scenario === 'network') process.exit(1)
console.log('HTTP/2.0 404 Not Found\\n\\n{"message":"Not Found"}')
process.exit(1)
`, { mode: 0o755 })
    for (const name of assets) fs.writeFileSync(path.join(dir, name), name)
    const manifest = stagedManifest()
    changeManifest(manifest)
    fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify(manifest))
    fs.writeFileSync(path.join(dir, 'embed-release-notes.md'), 'release notes')
    const result = spawnSync('bash', ['-euo', 'pipefail', '-c', script], { encoding: 'utf8', env: { ...process.env, PATH: `${dir}:${process.env.PATH}`, SCENARIO: scenario, ASSET_METADATA: JSON.stringify(assets.map(name => ({ name, size: Buffer.byteLength(name) }))), CALLS: path.join(dir, 'calls'), CREATED: path.join(dir, 'created'), PUBLISHED: path.join(dir, 'published'), RELEASE_ID: '42', TAG: `embed-${'a'.repeat(40)}`, GITHUB_SHA: 'a'.repeat(40), GITHUB_REPOSITORY: 'owner/repo', RUNNER_TEMP: dir, GITHUB_ENV: path.join(dir, 'env'), GH_TOKEN: 'mock', GITHUB_TOKEN: 'mock-github', RELEASE_ASSETS: assets.join(' '), STAGING: dir } })
    const authenticatedCalls = fs.readFileSync(path.join(dir, 'calls'), 'utf8').trim().split('\n').map(line => JSON.parse(line))
    return { ...result, githubEnv: fs.existsSync(path.join(dir, 'env')) ? fs.readFileSync(path.join(dir, 'env'), 'utf8') : '', calls: authenticatedCalls.map(call => call.args), authenticatedCalls }
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
    assert.equal(result.calls.some(args => args.includes('--method')), false)
  })
  test(`draft creation fails when recheck finds ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Create draft and upload all assets').status, 0)
  })
}

for (const scenario of ['upload-id-change', 'upload-published', 'upload-sha-change', 'upload-tag-change', 'upload-tag-created']) {
  test(`upload stops when draft ownership changes after recording: ${scenario}`, () => {
    const result = runReleaseStep(scenario, 'Create draft and upload all assets')
    assert.deepEqual(result.calls.filter(args => args.includes('--method')).map(args => args[args.indexOf('--method') + 1]), ['POST'])
  })
  test(`upload fails when draft ownership changes after recording: ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Create draft and upload all assets').status, 0)
  })
}

test('draft creation and upload succeed with confirmed ownership', () => {
  assert.equal(runReleaseStep('absent', 'Create draft and upload all assets').status, 0)
})

test('draft creation records the validated response id', () => {
  assert.equal(runReleaseStep('absent', 'Create draft and upload all assets').githubEnv, 'RELEASE_ID=42\n')
})

for (const scenario of ['id-zero', 'id-negative', 'id-string', 'id-fraction', 'id-unsafe', 'id-missing', 'created-published', 'created-tag', 'created-sha', 'created-json']) {
  test(`invalid create response fails: ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Create draft and upload all assets').status, 0)
  })
  test(`invalid create response prevents further writes: ${scenario}`, () => {
    const result = runReleaseStep(scenario, 'Create draft and upload all assets')
    assert.equal(result.calls.filter(args => args.includes('--method')).length, 1)
  })
  test(`invalid create response does not record an id: ${scenario}`, () => {
    assert.equal(runReleaseStep(scenario, 'Create draft and upload all assets').githubEnv, '')
  })
}

test('draft creation sends the exact REST fields and notes file', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets')
  const args = result.calls.find(args => args.includes('POST'))
  assert.deepEqual(args.map(arg => arg.startsWith('body=@') ? `body=@${path.basename(arg.slice(6))}` : arg), ['api', '--method', 'POST', 'repos/owner/repo/releases', '-f', `tag_name=embed-${'a'.repeat(40)}`, '-f', `target_commitish=${'a'.repeat(40)}`, '-f', `name=embed-${'a'.repeat(40)}`, '-F', 'draft=true', '-F', 'prerelease=false', '-F', 'body=@embed-release-notes.md'])
})

test('post-create ownership uses the response id even when the release list is empty', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets')
  const created = result.calls.findIndex(args => args.includes('POST'))
  assert.deepEqual(result.calls.slice(created + 1, created + 3).map(args => args.at(-1)), ['repos/owner/repo/releases/42', `repos/owner/repo/git/ref/tags/embed-${'a'.repeat(40)}`])
})

test('binary upload sizes match every staged file', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets')
  assert.deepEqual(result.authenticatedCalls.filter(call => call.args.includes('--input')).map(call => call.size), assets.map(name => Buffer.byteLength(name === 'manifest.json' ? JSON.stringify(stagedManifest()) : name)))
})

test('remote metadata verification reads the release by id', () => {
  assert.match(workflowSteps().find(step => step.startsWith('Verify uploaded assets\n')), /gh api "repos\/\$GITHUB_REPOSITORY\/releases\/\$RELEASE_ID" > "\$RUNNER_TEMP\/embed-release-assets.json"/)
})

test('draft upload uses exactly the ten ID-addressed binary uploads', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets')
  assert.deepEqual(result.calls.filter(args => args.some(a => a.startsWith('https://uploads.github.com/'))).map(args => ({ endpoint: args.find(a => a.startsWith('https://')), method: args[args.indexOf('--method') + 1], header: args[args.indexOf('-H') + 1], file: path.basename(args[args.indexOf('--input') + 1]) })), assets.map((name, index) => ({ endpoint: `https://uploads.github.com/repos/owner/repo/releases/42/assets?name=${encodeURIComponent(name)}`, method: 'POST', header: `Content-Type: ${uploadTypes[index]}`, file: name })))
})

test('payload upload uses the recorded type rather than inferring it from the extension', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets', manifest => { manifest.files[0].content_type = 'application/x-recorded-type' })
  assert.equal(result.calls.find(args => args.includes('--input'))?.find(arg => arg.startsWith('Content-Type:')), 'Content-Type: application/x-recorded-type')
})

for (const [name, change] of [
  ['missing payload', manifest => { manifest.files.shift() }],
  ['missing content type', manifest => { delete manifest.files[0].content_type }],
  ['empty content type', manifest => { manifest.files[0].content_type = '' }],
  ['whitespace content type', manifest => { manifest.files[0].content_type = '   ' }],
  ['non-string content type', manifest => { manifest.files[0].content_type = 42 }],
]) {
  test(`upload fails closed for ${name}`, () => {
    assert.notEqual(runReleaseStep('absent', 'Create draft and upload all assets', change).status, 0)
  })
  test(`invalid first payload prevents any uploads for ${name}`, () => {
    const result = runReleaseStep('absent', 'Create draft and upload all assets', change)
    assert.deepEqual(result.calls.filter(args => args.includes('--input')), [])
  })
}

test('a missing later payload type fails the upload step', () => {
  assert.notEqual(runReleaseStep('absent', 'Create draft and upload all assets', manifest => { delete manifest.files[7].content_type }).status, 0)
})

test('a missing later payload type prevents that upload and all remaining uploads', () => {
  const result = runReleaseStep('absent', 'Create draft and upload all assets', manifest => { delete manifest.files[7].content_type })
  assert.deepEqual(result.calls.filter(args => args.includes('--input')).map(args => path.basename(args[args.indexOf('--input') + 1])), assets.slice(0, 7))
})

test('every upload immediately follows ID ownership and tag absence reads', () => {
  const { calls } = runReleaseStep('absent', 'Create draft and upload all assets')
  assert.deepEqual(calls.flatMap((args, i) => args.some(a => a.startsWith('https://uploads.github.com/')) ? [calls.slice(i - 2, i).map(a => a.at(-1))] : []), assets.map(() => ['repos/owner/repo/releases/42', `repos/owner/repo/git/ref/tags/embed-${'a'.repeat(40)}`]))
})

test('a changed draft halfway through upload prevents remaining writes', () => {
  const result = runReleaseStep('midway-published', 'Create draft and upload all assets')
  assert.equal(result.calls.filter(args => args.some(a => a.startsWith('https://uploads.github.com/'))).length, 4)
})

test('a changed draft halfway through upload fails the step', () => {
  assert.notEqual(runReleaseStep('midway-published', 'Create draft and upload all assets').status, 0)
})

for (const [name, pattern] of [
  ['creation', /assert-absent\n          gh api --method POST/],
  ['post-create ownership', /NODE\n          \)\n          GH_TOKEN="\$GITHUB_TOKEN" node apps\/web\/scripts\/embed-release-guard.mjs assert-own-draft --release-id "\$RELEASE_ID"/],
  ['publication', /assert-own-draft --release-id "\$RELEASE_ID"\n          gh api --method PATCH/],
  ['post-publication', /gh api --method PATCH[^\n]+\n          GH_TOKEN="\$GITHUB_TOKEN" node apps\/web\/scripts\/embed-release-guard.mjs assert-published/],
]) {
  test(`release guard orders ${name}`, () => assert.match(workflow(), pattern))
}

for (const scenario of ['tag', 'release', 'absent', 'auth', 'network', 'list-error', 'bad-json']) {
  test(`publication makes no release writes when recheck finds ${scenario}`, () => {
    const result = runReleaseStep(scenario, 'Publish prerelease')
    assert.equal(result.calls.some(args => args.includes('--method')), false)
  })
  test(`publication fails when recheck finds ${scenario}`, () => {
    assert.notEqual(runReleaseStep(scenario, 'Publish prerelease').status, 0)
  })
}

test('publication succeeds with a matching published tag', () => {
  assert.equal(runReleaseStep('draft', 'Publish prerelease').status, 0)
})

test('publication verifies the created tag after PATCH', () => {
  const result = runReleaseStep('draft', 'Publish prerelease')
  assert.deepEqual(result.calls.map(args => args.slice(0, 2)), [['api', '--include'], ['api', '--include'], ['api', '--method'], ['api', '--include']])
})

test('publication fails for a wrong tag commit', () => {
  assert.notEqual(runReleaseStep('publish-wrong', 'Publish prerelease').status, 0)
})

test('publication reports a wrong tag commit', () => {
  assert.match(runReleaseStep('publish-wrong', 'Publish prerelease').stderr, /matches GITHUB_SHA: false/)
})

test('publication makes no further writes for a wrong tag commit', () => {
  const result = runReleaseStep('publish-wrong', 'Publish prerelease')
  assert.deepEqual(result.calls.filter(args => args.includes('--method')).map(args => args[args.indexOf('--method') + 1]), ['PATCH'])
})

function releaseWriteGuards(source) {
  const lines = source.replace(/\\\n\s*/g, ' ').split('\n').map(line => line.trim())
  return lines.flatMap((line, index) => /\bgh (?:release\b|api\b.*(?:--method|-X|-f\b|-F\b|--field|--raw-field|--input))/.test(line) ? [lines[index - 1]] : [])
}

test('every release write has its corresponding guard on the immediately preceding line', () => {
  assert.deepEqual(releaseWriteGuards(workflow()), ['assert-absent', 'assert-own-draft --release-id "$RELEASE_ID"', 'assert-own-draft --release-id "$RELEASE_ID"'].map(guard => `GH_TOKEN="$GITHUB_TOKEN" node apps/web/scripts/embed-release-guard.mjs ${guard}`))
})

for (const command of [
  'gh api -X POST repos/owner/repo/git/refs',
  'gh api --method=PUT repos/owner/repo/git/refs',
  'gh api repos/owner/repo/git/refs -f ref=refs/tags/test -f sha=test',
  'gh api repos/owner/repo/git/refs \\\n    -F ref=refs/tags/test',
  'gh release upload test asset',
  'ref=$(gh api -X POST repos/owner/repo/git/refs)',
  '- run: gh api -X POST repos/owner/repo/git/refs',
  'GH_TOKEN=mock gh api -X POST repos/owner/repo/git/refs',
  'if gh api -X POST repos/owner/repo/git/refs; then',
]) {
  test(`write detection includes alternate syntax: ${command}`, () => {
    assert.equal(releaseWriteGuards(`${workflow()}\n${command}`).length, 4)
  })
}

function workflowSteps() {
  return workflow().split('      - name: ').slice(1)
}

const appToken = '${{ steps.app-token.outputs.token }}'

test('release App token has exactly the verified v3 inputs and contents permission', () => {
  const step = workflowSteps().find(step => step.includes('id: app-token')) || ''
  assert.deepEqual(step.match(/^          [\w-]+: .+$/gm), [
    '          client-id: ${{ vars.EMBED_RELEASE_APP_CLIENT_ID }}',
    '          private-key: ${{ secrets.EMBED_RELEASE_APP_PRIVATE_KEY }}',
    '          owner: ${{ github.repository_owner }}',
    '          repositories: qni-webgpu',
    '          permission-contents: write',
  ])
})

test('release App token uses v3 after read-only pre-write checks and before draft creation', () => {
  assert.match(workflow(), /- name: Recheck tag and release absence[\s\S]*?assert-absent\n\n      - name: Create Release App token\n        id: app-token\n        uses: actions\/create-github-app-token@v3[\s\S]*?gh api --method POST/)
})

test('release App configuration is checked before checkout and the build', () => {
  assert.match(workflow(), /steps:\n      - name: Check Release App configuration\n        env:\n          CLIENT_ID: \$\{\{ vars.EMBED_RELEASE_APP_CLIENT_ID \}\}\n          HAS_KEY: \$\{\{ secrets.EMBED_RELEASE_APP_PRIVATE_KEY != '' \}\}\n[\s\S]*?- name: Checkout[\s\S]*?build:\n    needs: check/)
})

for (const [name, env, message] of [
  ['client ID', { CLIENT_ID: '', HAS_KEY: 'true' }, 'EMBED_RELEASE_APP_CLIENT_ID repository variable is not set'],
  ['private key', { CLIENT_ID: 'client', HAS_KEY: 'false' }, 'EMBED_RELEASE_APP_PRIVATE_KEY secret is not set'],
]) {
  test(`missing Release App ${name} fails clearly`, () => {
    const step = workflowSteps().find(step => step.startsWith('Check Release App configuration\n')) || ''
    const script = step.split('        run: |\n')[1] || ''
    const result = spawnSync('bash', ['-c', script], { encoding: 'utf8', env: { ...process.env, ...env } })
    assert.deepEqual({ status: result.status, stderr: result.stderr.trim() }, { status: 1, stderr: `::error::${message}` })
  })
}

test('valid Release App configuration succeeds', () => {
  const step = workflowSteps().find(step => step.startsWith('Check Release App configuration\n')) || ''
  assert.equal(spawnSync('bash', ['-c', step.split('        run: |\n')[1] || 'exit 1'], { env: { ...process.env, CLIENT_ID: 'client', HAS_KEY: 'true' } }).status, 0)
})

test('every release write step uses only the App token for gh writes', () => {
  const writes = workflowSteps().filter(step => /gh api --method (POST|PATCH) /.test(step))
  assert.deepEqual(writes.map(step => step.match(/GH_TOKEN: (.+)/)?.[1]), [appToken, appToken])
})

for (const [step, scenario] of [['Create draft and upload all assets', 'absent'], ['Publish prerelease', 'draft']]) {
  test(`${step} executes every write with the App token and every guard read with the GitHub token`, () => {
    const { authenticatedCalls } = runReleaseStep(scenario, step)
    assert.deepEqual(authenticatedCalls.map(call => call.token), authenticatedCalls.map(call => call.args.includes('--method') ? 'mock' : 'mock-github'))
  })
}

test('read steps retain the GitHub token', () => {
  assert.deepEqual(workflowSteps().filter(step => /^(Check tag and release absence|Recheck tag and release absence|Verify uploaded assets)\n/.test(step)).map(step => step.match(/GH_TOKEN: (.+)/)?.[1]), Array(3).fill('${{ github.token }}'))
})

test('tokens are scoped only to the five gh steps', () => {
  assert.deepEqual(workflow().split('\n').filter(line => line.includes('GH_TOKEN:')), [
    '          GH_TOKEN: ${{ github.token }}',
    '          GH_TOKEN: ${{ github.token }}',
    `          GH_TOKEN: ${appToken}`,
    '          GH_TOKEN: ${{ github.token }}',
    `          GH_TOKEN: ${appToken}`,
  ])
})

test('attestation does not receive an App token', () => {
  assert.doesNotMatch(workflowSteps().find(step => step.startsWith('Attest release files\n')), /app-token|github-token:|GH_TOKEN:/)
})

test('only embed release workflow references the Release App private key', () => {
  const directory = path.dirname(workflowPath)
  assert.deepEqual(fs.readdirSync(directory).filter(name => /\.ya?ml$/.test(name) && fs.readFileSync(path.join(directory, name), 'utf8').includes('EMBED_RELEASE_APP_PRIVATE_KEY')), ['embed-release.yml'])
})
