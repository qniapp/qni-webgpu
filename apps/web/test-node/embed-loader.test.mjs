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
    export const starts = { active: 0, max: 0, args: [] };
    export async function start_embed(canvas, circuit, showStatePanel, palette) {
      starts.args.push({ showStatePanel, palette });
      starts.active++;
      starts.max = Math.max(starts.max, starts.active);
      await new Promise(resolve => setTimeout(resolve, 1));
      starts.active--;
      if (canvas.fail) throw new Error('startup failed');
      return {destroy(){}, free(){}, read_state_vector(){ return Promise.resolve(canvas.state) }, circuit_json(){ return canvas.circuit ?? circuit }};
    }
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
  const { starts } = await import(pathToFileURL(join(dir, 'qni-web.js')).href)
  return { module, starts, requests: () => requests }
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

test('seven concurrent elements serialize GPU startup and fetch wasm once', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  const handles = await Promise.all(Array.from({ length: 7 }, () => f.module.startEmbed({}, '{}')))
  handles.forEach(handle => handle.destroy())
  assert.deepEqual({ maximumConcurrentStarts: f.starts.max, wasmRequests: f.requests() }, { maximumConcurrentStarts: 1, wasmRequests: 1 })
})

test('a failed runner does not poison later startup or instance readback', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  await f.module.startEmbed({ fail: true }, '{}').catch(() => {})
  const handle = await f.module.startEmbed({ state: [7] }, '{}')
  const state = await handle.readStateVector()
  handle.destroy()
  assert.deepEqual(state, [7])
})

test('circuit export is synchronous and scoped to the current runner', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  const canvas = {}
  const first = await f.module.startEmbed(canvas, '{"cols":[["H"]]}')
  const second = await f.module.startEmbed({}, '{"cols":[["X"]]}')
  canvas.circuit = '{"cols":[["T"]]}'
  const circuits = [first.circuitJSON(), second.circuitJSON()]
  first.destroy()
  second.destroy()
  assert.deepEqual(circuits, ['{"cols":[["T"]]}', '{"cols":[["X"]]}'])
})

test('palette setting reaches the wasm entry unchanged', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  await f.module.startEmbed({}, '{}', { palette: ['|0>', '|1>', 'H'] })
  assert.deepEqual(f.starts.args.at(-1).palette, ['|0>', '|1>', 'H'])
})

test('omitted palette keeps the full palette', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  await f.module.startEmbed({}, '{}')
  assert.equal(f.starts.args.at(-1).palette, undefined)
})
