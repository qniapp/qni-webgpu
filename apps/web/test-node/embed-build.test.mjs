import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL, fileURLToPath } from 'node:url'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'

const exec = promisify(execFile)

class EmbedBuild {
  static async sourceCommit(t, variables, { noGit = false } = {}) {
    const dir = await mkdtemp(join(tmpdir(), 'qni-build-contract-'))
    t.after(() => rm(dir, { recursive: true, force: true }))
    const bin = join(dir, 'bin')
    await mkdir(bin)
    // Only replace compilation; execute the real packaging and SHA injection.
    await writeFile(join(bin, 'trunk'), `#!/usr/bin/env bash
while [[ "$1" != "--dist" ]]; do shift; done
mkdir -p "$2"
printf 'export default function() {}' > "$2/qni-web.js"
printf wasm > "$2/qni-web_bg.wasm"
`, { mode: 0o755 })
    if (noGit) await writeFile(join(bin, 'git'), '#!/usr/bin/env bash\nexit 128\n', { mode: 0o755 })
    const env = { ...process.env, PATH: `${bin}:${process.env.PATH}` }
    delete env.QNI_SOURCE_COMMIT
    delete env.GITHUB_SHA
    Object.assign(env, variables)
    const output = join(dir, 'bundle')
    await exec('bash', [fileURLToPath(new URL('../scripts/build-embed.sh', import.meta.url)), output], { env })
    return (await import(pathToFileURL(join(output, 'qni-embed.mjs')).href)).SOURCE_COMMIT
  }
}

test('build prefers the explicit source commit over GitHub SHA', async t => {
  assert.equal(await EmbedBuild.sourceCommit(t, { QNI_SOURCE_COMMIT: 'a'.repeat(40), GITHUB_SHA: 'b'.repeat(40) }), 'a'.repeat(40))
})

test('build uses GitHub SHA when no explicit commit is provided', async t => {
  assert.equal(await EmbedBuild.sourceCommit(t, { GITHUB_SHA: 'b'.repeat(40) }), 'b'.repeat(40))
})

test('build falls back to the checkout commit', async t => {
  const { stdout } = await exec('git', ['rev-parse', 'HEAD'], { cwd: fileURLToPath(new URL('../../../', import.meta.url)) })
  assert.equal(await EmbedBuild.sourceCommit(t, {}), stdout.trim())
})

test('source archive builds without git use unknown provenance', async t => {
  assert.equal(await EmbedBuild.sourceCommit(t, {}, { noGit: true }), 'unknown')
})

test('build cannot inject executable text through an invalid SHA', async t => {
  assert.equal(await EmbedBuild.sourceCommit(t, { QNI_SOURCE_COMMIT: "';throw Error('injected');//" }), 'unknown')
})
