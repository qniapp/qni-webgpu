const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const os = require('node:os')
const path = require('node:path')
const { createHash } = require('node:crypto')

const root = path.join(__dirname, '..')
const sha = 'a'.repeat(40)
const versions = { rustc: 'rustc test', 'wasm-bindgen': '0.2.129', 'wasm-opt': 'version_123', trunk: 'trunk test', node: 'v22.0.0', pnpm: '9.15.9' }
const names = ['qni-web.js', 'qni-web_bg.wasm', 'qni-embed.mjs', 'qni-embed-error.mjs', 'h.svg', 'design-system.css', 'h-gate.js', 'LICENSE']
const hash = bytes => createHash('sha256').update(bytes).digest('hex')

class Fixture {
  static async create(t, commit = sha) {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'qni-release-test-'))
    t.after(() => fs.rm(directory, { recursive: true, force: true }))
    const input = path.join(directory, 'input')
    const output = path.join(directory, 'output')
    await fs.mkdir(input)
    await fs.writeFile(path.join(input, 'qni-web.js'), '// generated glue\n')
    await fs.writeFile(path.join(input, 'qni-web_bg.wasm'), Buffer.from([0, 97, 115, 109]))
    const source = await fs.readFile(path.join(root, 'apps/web/embed.mjs'), 'utf8')
    await fs.writeFile(path.join(input, 'qni-embed.mjs'), source.replace('__QNI_SOURCE_COMMIT__', commit).replace('EMBED_API_VERSION = 1', 'EMBED_API_VERSION = 37'))
    await fs.copyFile(path.join(root, 'apps/web/qni-embed-error.mjs'), path.join(input, 'qni-embed-error.mjs'))
    const { EmbedRelease } = await import('../apps/web/scripts/embed-release.mjs')
    return { input, output, stage: (options = {}) => EmbedRelease.stage({ input, output, sourceCommit: sha, toolchain: { ...versions }, ...options }), verify: () => EmbedRelease.verify(output) }
  }
}

test('manifest records the schema and source identity', async t => {
  const fixture = await Fixture.create(t)
  const manifest = await fixture.stage()
  assert.deepEqual([manifest.schema_version, manifest.source_commit], [1, sha])
})

test('manifest reads the API version from the built module', async t => {
  const fixture = await Fixture.create(t)
  assert.equal((await fixture.stage()).embed_api_version, 37)
})

test('metadata extraction never executes the built module', async t => {
  const fixture = await Fixture.create(t)
  await fs.appendFile(path.join(fixture.input, 'qni-embed.mjs'), '\nthrow new Error("Do not execute release files")\n')
  assert.equal((await fixture.stage()).embed_api_version, 37)
})

test('stage rejects a changed SOURCE_COMMIT export expression', async t => {
  const fixture = await Fixture.create(t)
  const filename = path.join(fixture.input, 'qni-embed.mjs')
  const source = await fs.readFile(filename, 'utf8')
  await fs.writeFile(filename, source.replace("? buildSourceCommit : 'unknown'", "? 'unknown' : buildSourceCommit"))
  await assert.rejects(fixture.stage(), /SOURCE_COMMIT/)
})

test('pinned wasm tool versions are read from repository configuration', async () => {
  const { EmbedRelease } = await import('../apps/web/scripts/embed-release.mjs')
  assert.deepEqual(await EmbedRelease.pinnedWasmTools(), { 'wasm-bindgen': '0.2.129', 'wasm-opt': 'version_123' })
})

for (const args of [['verify'], ['invalid', '--output', 'unused'], ['verify', '--output', 'unused', 'extra']]) {
  test(`CLI rejects invalid arguments ${args.join(' ')}`, async () => {
    const { EmbedRelease } = await import('../apps/web/scripts/embed-release.mjs')
    await assert.rejects(EmbedRelease.main(args), /usage/)
  })
}

test('CLI verify rejects a different expected source commit', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  const { EmbedRelease } = await import('../apps/web/scripts/embed-release.mjs')
  await assert.rejects(EmbedRelease.main(['verify', '--output', fixture.output, '--source-commit', 'b'.repeat(40)]), /source_commit mismatch/)
})

test('CLI verify checks an existing staged release', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  const { EmbedRelease } = await import('../apps/web/scripts/embed-release.mjs')
  assert.equal((await EmbedRelease.main(['verify', '--output', fixture.output])).source_commit, sha)
})

test('verify rejects symbolic links even to identical content', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  const filename = path.join(fixture.output, 'h.svg')
  const target = path.join(fixture.input, 'identical.svg')
  await fs.rename(filename, target)
  await fs.symlink(target, filename)
  await assert.rejects(fixture.verify(), /regular file/)
})

test('manifest records UTC build time', async t => {
  const fixture = await Fixture.create(t)
  assert.match((await fixture.stage()).built_at, /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{3})?Z$/)
})

test('manifest records all toolchain versions', async t => {
  const fixture = await Fixture.create(t)
  assert.deepEqual((await fixture.stage()).toolchain, versions)
})

test('manifest lists the ordered flat asset set', async t => {
  const fixture = await Fixture.create(t)
  assert.deepEqual((await fixture.stage()).files.map(file => file.name), names)
})

test('manifest records hashes and sizes of every asset', async t => {
  const fixture = await Fixture.create(t)
  const manifest = await fixture.stage()
  const actual = await Promise.all(manifest.files.map(async file => {
    const bytes = await fs.readFile(path.join(fixture.output, file.name))
    return { name: file.name, sha256: hash(bytes), size: bytes.length }
  }))
  assert.deepEqual(manifest.files.map(({ name, sha256, size }) => ({ name, sha256, size })), actual)
})

test('manifest records MIME types', async t => {
  const fixture = await Fixture.create(t)
  assert.deepEqual((await fixture.stage()).files.map(file => file.content_type), ['text/javascript', 'application/wasm', 'text/javascript', 'text/javascript', 'image/svg+xml', 'text/css', 'text/javascript', 'text/plain'])
})

test('manifest checksum uses sha256sum format', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  assert.equal(await fs.readFile(path.join(fixture.output, 'manifest.json.sha256'), 'utf8'), `${hash(await fs.readFile(path.join(fixture.output, 'manifest.json')))}  manifest.json\n`)
})

test('stage rejects a mismatched built SOURCE_COMMIT', async t => {
  const fixture = await Fixture.create(t, 'b'.repeat(40))
  await assert.rejects(fixture.stage(), /SOURCE_COMMIT/)
})

for (const sourceCommit of ['short', '../escape', 'unknown']) {
  test(`stage rejects invalid source SHA ${sourceCommit}`, async t => {
    const fixture = await Fixture.create(t)
    await assert.rejects(fixture.stage({ sourceCommit }), /40.*hex/i)
  })
}

test('stage refuses a nonempty output directory', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  await assert.rejects(fixture.stage(), /empty/)
})

test('verify accepts an intact release', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  assert.equal((await fixture.verify()).source_commit, sha)
})

for (const name of [...names, 'manifest.json', 'manifest.json.sha256']) {
  test(`verify detects tampering with ${name}`, async t => {
    const fixture = await Fixture.create(t)
    await fixture.stage()
    await fs.appendFile(path.join(fixture.output, name), 'tampered')
    await assert.rejects(fixture.verify(), /mismatch|checksum/i)
  })
}

test('verify rejects missing assets', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  await fs.unlink(path.join(fixture.output, 'h.svg'))
  await assert.rejects(fixture.verify(), /asset|ENOENT/)
})

test('verify rejects unlisted assets', async t => {
  const fixture = await Fixture.create(t)
  await fixture.stage()
  await fs.writeFile(path.join(fixture.output, 'unexpected.js'), '')
  await assert.rejects(fixture.verify(), /asset/)
})

test('stage rejects a noninteger API version exported by the built module', async t => {
  const fixture = await Fixture.create(t)
  const filename = path.join(fixture.input, 'qni-embed.mjs')
  await fs.writeFile(filename, (await fs.readFile(filename, 'utf8')).replace('EMBED_API_VERSION = 37', 'EMBED_API_VERSION = 1.5'))
  await assert.rejects(fixture.stage(), /EMBED_API_VERSION/)
})

for (const [name, change, pattern] of [
  ['schema version', manifest => { manifest.schema_version = 2 }, /schema/],
  ['API version', manifest => { manifest.embed_api_version = 99 }, /embed_api_version/],
  ['build timestamp', manifest => { manifest.built_at = 'yesterday' }, /schema/],
  ['toolchain version', manifest => { delete manifest.toolchain.rustc }, /toolchain/],
  ['source identity', manifest => { manifest.source_commit = 'b'.repeat(40) }, /SOURCE_COMMIT/],
  ['duplicate asset name', manifest => { manifest.files[1].name = manifest.files[0].name }, /asset/],
  ['asset MIME type', manifest => { manifest.files[0].content_type = 'text/plain' }, /metadata/],
  ['asset byte count', manifest => { manifest.files[0].size += 1 }, /metadata/],
]) {
  test(`verify rejects invalid ${name} with a recomputed manifest checksum`, async t => {
    const fixture = await Fixture.create(t)
    const manifest = await fixture.stage()
    change(manifest)
    const bytes = JSON.stringify(manifest)
    await fs.writeFile(path.join(fixture.output, 'manifest.json'), bytes)
    await fs.writeFile(path.join(fixture.output, 'manifest.json.sha256'), `${hash(bytes)}  manifest.json\n`)
    await assert.rejects(fixture.verify(), pattern)
  })
}

test('verify rejects traversal even with a recomputed manifest checksum', async t => {
  const fixture = await Fixture.create(t)
  const manifest = await fixture.stage()
  manifest.files[0].name = '../outside'
  const bytes = JSON.stringify(manifest)
  await fs.writeFile(path.join(fixture.output, 'manifest.json'), bytes)
  await fs.writeFile(path.join(fixture.output, 'manifest.json.sha256'), `${hash(bytes)}  manifest.json\n`)
  await assert.rejects(fixture.verify(), /asset/)
})
