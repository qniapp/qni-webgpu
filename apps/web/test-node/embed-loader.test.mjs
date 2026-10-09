import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, writeFile, copyFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

async function fixture(t, headers, fail = false) {
  const dir = await mkdtemp(join(tmpdir(), 'qni-embed-test-'))
  t.after(() => rm(dir, { recursive: true, force: true }))
  await copyFile(new URL('../embed.mjs', import.meta.url), join(dir, 'embed.mjs'))
  await writeFile(join(dir, 'qni-web.js'), `
    export default async function init({module_or_path}) {
      if (!(module_or_path instanceof Response)) throw new Error('Expected streaming Response');
      if (module_or_path.headers.get('content-type') !== 'application/wasm') throw new Error('Lost MIME type');
      await module_or_path.arrayBuffer();
    }
    export async function start_embed() { return {destroy(){}, free(){}} }
  `)
  await writeFile(join(dir, 'package.json'), '{"type":"module"}')
  const original = globalThis.fetch
  let requests = 0
  globalThis.fetch = async () => {
    requests++
    return new Response(new Uint8Array(8), { headers, status: fail && requests === 1 ? 500 : 200 })
  }
  t.after(() => { globalThis.fetch = original })
  const module = await import(pathToFileURL(join(dir, 'embed.mjs')).href)
  return { module, requests: () => requests }
}

test('early preparation and canvas startup share one streaming fetch', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm', 'content-length': '8' })
  const progress = []
  await Promise.all([f.module.prepareEmbed(), f.module.startEmbed({}, '{}', { onProgress: p => progress.push(p) })])
  assert.deepEqual({ requests: f.requests(), downloaded: progress.find(p => p.loaded === 8 && p.stage === 'download') }, { requests: 1, downloaded: { stage: 'download', loaded: 8, total: 8 } })
})

test('compressed length is never treated as decoded progress total', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm', 'content-length': '4', 'content-encoding': 'gzip' })
  const progress = []
  await f.module.startEmbed({}, '{}', { onProgress: p => progress.push(p) })
  assert.equal(progress.find(p => p.loaded === 8 && p.stage === 'download').total, null)
})

test('failed early preparation can retry on canvas startup', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' }, true)
  await f.module.prepareEmbed().catch(() => {})
  await f.module.startEmbed({}, '{}')
  assert.equal(f.requests(), 2)
})
