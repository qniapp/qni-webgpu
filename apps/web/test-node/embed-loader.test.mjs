// Keep the established preflight entry point while including the independent error API tests.
import './embed-startup-error.test.mjs'
import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, writeFile, copyFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'

async function fixture(t, headers = { 'content-type': 'application/wasm' }, fail = false, options = {}) {
  const dir = await mkdtemp(join(tmpdir(), 'qni-embed-test-'))
  t.after(() => rm(dir, { recursive: true, force: true }))
  await copyFile(new URL('../embed.mjs', import.meta.url), join(dir, 'embed.mjs'))
  await copyFile(new URL('../qni-embed-error.mjs', import.meta.url), join(dir, 'qni-embed-error.mjs'))
  await writeFile(join(dir, 'qni-web.js'), `
    export default async function init({module_or_path}) {
      if (!(module_or_path instanceof Response)) throw new Error('Expected streaming Response');
      if (module_or_path.headers.get('content-type') !== 'application/wasm') throw new Error('Lost MIME type');
      await module_or_path.arrayBuffer();
      ${options.compileError ? "throw new WebAssembly.CompileError('bad wasm');" : ''}
    }
    export const starts = { args: [] };
    export async function start_embed(canvas, circuit, showStatePanel, palette, maxWireCount, showRestartButton, onRestart) {
      starts.args.push({ showStatePanel, palette, maxWireCount, showRestartButton, onRestart });
      await new Promise(resolve => setTimeout(resolve, 1));
      if (canvas.failWith) throw Object.assign(new Error(canvas.failWith.message), { code: canvas.failWith.code });
      if (canvas.never) await new Promise(() => {});
      if (canvas.late) await new Promise(resolve => setTimeout(resolve, canvas.late));
      if (canvas.lost) globalThis.dispatchEvent(Object.assign(new Event('qni-device-lost'), { detail: canvas.lost }));
      if (canvas.fail) throw new Error('startup failed');
      return {destroy(){ canvas.destroyed = (canvas.destroyed ?? 0) + 1 }, free(){ canvas.freed = (canvas.freed ?? 0) + 1 }, read_state_vector(){ return Promise.resolve(canvas.state) }, circuit_json(){ return canvas.circuit ?? circuit }};
    }
  `)
  await writeFile(join(dir, 'package.json'), '{"type":"module"}')
  const saved = new Map()
  const setGlobal = (key, value) => {
    saved.set(key, Object.getOwnPropertyDescriptor(globalThis, key))
    Object.defineProperty(globalThis, key, { configurable: true, writable: true, value })
  }
  let probes = 0
  const thrown = new Error('adapter blocked')
  const gpu = options.gpu === 'missing' ? undefined : options.gpu === 'no-request-adapter' ? {} : {
    async requestAdapter() {
      probes++
      await Promise.resolve()
      if (options.gpu === 'never') await new Promise(() => {})
      if (options.gpu === 'throws') throw thrown
      return options.gpu === 'null' ? null : {}
    },
  }
  setGlobal('navigator', { gpu, onLine: options.offline !== true })
  setGlobal('isSecureContext', options.secure)
  const target = new EventTarget()
  if (typeof globalThis.addEventListener !== 'function') {
    for (const key of ['addEventListener', 'removeEventListener', 'dispatchEvent']) setGlobal(key, target[key].bind(target))
  }
  t.after(() => {
    for (const [key, descriptor] of saved) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor)
      else delete globalThis[key]
    }
  })
  const original = globalThis.fetch
  let requests = 0
  globalThis.fetch = async () => {
    requests++
    if (options.fetchThrows) throw new TypeError('network down')
    if (options.downloadDelay) await new Promise(resolve => setTimeout(resolve, options.downloadDelay))
    return new Response(new Uint8Array(8), { headers, status: options.status ?? (fail && requests === 1 ? 500 : 200) })
  }
  t.after(() => { globalThis.fetch = original })
  const module = await import(pathToFileURL(join(dir, 'embed.mjs')).href)
  const { starts } = await import(pathToFileURL(join(dir, 'qni-web.js')).href)
  if (options.noWasm) setGlobal('WebAssembly', undefined)
  return { module, starts, thrown, probes: () => probes, requests: () => requests }
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

test('seven concurrent elements all start and fetch wasm once', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  const handles = await Promise.all(Array.from({ length: 7 }, () => f.module.startEmbed({}, '{}')))
  handles.forEach(handle => handle.destroy())
  assert.deepEqual({ started: handles.length, runnerStarts: f.starts.args.length, wasmRequests: f.requests() }, { started: 7, runnerStarts: 7, wasmRequests: 1 })
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

test('maxWireCount setting reaches the wasm entry unchanged', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  await f.module.startEmbed({}, '{}', { maxWireCount: 1 })
  assert.equal(f.starts.args.at(-1).maxWireCount, 1)
})

test('omitted maxWireCount keeps the default wire count', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  await f.module.startEmbed({}, '{}')
  assert.equal(f.starts.args.at(-1).maxWireCount, undefined)
})

for (const settings of [{}, { showRestartButton: true }, { onRestart() {} }]) {
  test(`restart action needs both opt-in and callback: ${JSON.stringify(settings)}`, async t => {
    const f = await fixture(t, { 'content-type': 'application/wasm' })
    await f.module.startEmbed({}, '{}', settings)
    assert.equal(f.starts.args.at(-1).onRestart, undefined)
  })
}

test('restart notification is deferred so the host can export and destroy safely', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  const events = []
  const handle = await f.module.startEmbed({}, '{}', { showRestartButton: true, onRestart() {
    events.push(handle.circuitJSON())
    handle.destroy()
  } })
  f.starts.args.at(-1).onRestart()
  events.push('egui returned')
  await Promise.resolve()
  assert.deepEqual(events, ['egui returned', '{}'])
})

test('device loss cancels a pending restart notification', async t => {
  const target = new EventTarget()
  const originalAdd = globalThis.addEventListener
  const originalRemove = globalThis.removeEventListener
  globalThis.addEventListener = target.addEventListener.bind(target)
  globalThis.removeEventListener = target.removeEventListener.bind(target)
  t.after(() => {
    globalThis.addEventListener = originalAdd
    globalThis.removeEventListener = originalRemove
  })
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  let calls = 0
  const handle = await f.module.startEmbed({}, '{}', { showRestartButton: true, onRestart() { calls++ } })
  f.starts.args.at(-1).onRestart()
  target.dispatchEvent(new Event('qni-device-lost'))
  await Promise.resolve()
  handle.destroy()
  assert.equal(calls, 0)
})

test('destroy cancels a pending restart notification', async t => {
  const f = await fixture(t, { 'content-type': 'application/wasm' })
  let calls = 0
  const handle = await f.module.startEmbed({}, '{}', { showRestartButton: true, onRestart() { calls++ } })
  f.starts.args.at(-1).onRestart()
  handle.destroy()
  await Promise.resolve()
  assert.equal(calls, 0)
})

for (const [name, options, code] of [
  ['missing GPU', { gpu: 'missing' }, 'webgpu-unsupported'],
  ['GPU without requestAdapter', { gpu: 'no-request-adapter' }, 'webgpu-unsupported'],
  ['insecure context', { secure: false }, 'insecure-context'],
  ['insecure before missing GPU', { secure: false, gpu: 'missing' }, 'insecure-context'],
  ['missing WebAssembly', { noWasm: true }, 'wasm-unavailable'],
  ['null adapter', { gpu: 'null' }, 'gpu-adapter-unavailable'],
  ['throwing adapter', { gpu: 'throws' }, 'gpu-adapter-unavailable'],
  ['wasm HTTP 404', { status: 404 }, 'load-failed'],
  ['network failure', { fetchThrows: true }, 'load-failed'],
  ['wasm compile failure', { compileError: true }, 'load-failed'],
]) {
  test(`${name} has a stable startup code`, async t => {
    const f = await fixture(t, undefined, false, options)
    assert.equal((await f.module.startEmbed({}, '{}').catch(e => e)).code, code)
  })
}

test('missing GPU never downloads wasm', async t => {
  const f = await fixture(t, undefined, false, { gpu: 'missing' })
  await f.module.startEmbed({}, '{}').catch(() => {})
  assert.equal(f.requests(), 0)
})

test('undefined secure context permits startup', async t => {
  const f = await fixture(t)
  assert.equal(typeof (await f.module.startEmbed({}, '{}')).destroy, 'function')
})

for (const [name, options, field, expected] of [
  ['null adapter reason', { gpu: 'null' }, 'reason', 'null'],
  ['HTTP status', { status: 404 }, 'status', 404],
  ['offline download', { fetchThrows: true, offline: true }, 'offline', true],
]) {
  test(name, async t => {
    const f = await fixture(t, undefined, false, options)
    assert.equal((await f.module.startEmbed({}, '{}').catch(e => e)).detail[field], expected)
  })
}

test('adapter exception is retained as cause', async t => {
  const f = await fixture(t, undefined, false, { gpu: 'throws' })
  assert.equal((await f.module.startEmbed({}, '{}').catch(e => e)).cause, f.thrown)
})

test('concurrent startups share adapter probe', async t => {
  const f = await fixture(t)
  await Promise.all([f.module.startEmbed({}, '{}'), f.module.startEmbed({}, '{}')])
  assert.equal(f.probes(), 1)
})

test('failed adapter probe is retried', async t => {
  const f = await fixture(t, undefined, false, { gpu: 'null' })
  await f.module.startEmbed({}, '{}').catch(() => {})
  await f.module.startEmbed({}, '{}').catch(() => {})
  assert.equal(f.probes(), 2)
})

for (const code of ['gpu-adapter-unavailable', 'gpu-device-unavailable', 'invalid-embed', 'not-a-code']) {
  test(`Rust ${code} is classified`, async t => {
    const f = await fixture(t)
    assert.equal((await f.module.startEmbed({ failWith: { code, message: 'Rust failure' } }, '{}').catch(e => e)).code,
      code === 'not-a-code' ? 'unknown' : code)
  })
}

for (const message of ['invalid circuit JSON: bad token', 'unknown palette gate: Foo', 'maxWireCount must be a positive integer: 0']) {
  test(`invalid embed preserves ${message}`, async t => {
    const f = await fixture(t)
    assert.equal((await f.module.startEmbed({ failWith: { code: 'invalid-embed', message } }, '{}').catch(e => e)).message, message)
  })
}

test('plain wasm runner Error becomes unknown', async t => {
  const f = await fixture(t)
  assert.equal((await f.module.startEmbed({ fail: true }, '{}').catch(e => e)).code, 'unknown')
})

test('device loss during startup is classified', async t => {
  const f = await fixture(t)
  assert.equal((await f.module.startEmbed({ lost: { reason: 'Unknown', message: 'lost' } }, '{}').catch(e => e)).code, 'gpu-device-lost')
})

test('device loss during startup retains detail', async t => {
  const f = await fixture(t)
  assert.equal((await f.module.startEmbed({ lost: { reason: 'Unknown' } }, '{}').catch(e => e)).detail.reason, 'Unknown')
})

test('onDeviceLost receives device loss detail', async t => {
  const f = await fixture(t)
  let received
  const handle = await f.module.startEmbed({}, '{}', { onDeviceLost: e => { received = e } })
  globalThis.dispatchEvent(Object.assign(new Event('qni-device-lost'), { detail: { reason: 'Unknown', message: 'driver reset' } }))
  handle.destroy()
  assert.deepEqual(received.detail, { reason: 'Unknown', message: 'driver reset' })
})

test('frozen GPU startup times out', async t => {
  const f = await fixture(t)
  assert.equal((await f.module.startEmbed({ never: true }, '{}', { startupTimeoutMs: 5 }).catch(e => e)).code, 'startup-timeout')
})

for (const property of ['destroyed', 'freed']) {
  test(`late runner is ${property} after timeout`, async t => {
    const f = await fixture(t)
    const canvas = { late: 25 }
    await f.module.startEmbed(canvas, '{}', { startupTimeoutMs: 5 }).catch(() => {})
    await new Promise(resolve => setTimeout(resolve, 40))
    assert.equal(canvas[property], 1)
  })
}

test('download time is excluded from GPU timeout', async t => {
  const f = await fixture(t, undefined, false, { downloadDelay: 25 })
  assert.equal(typeof (await f.module.startEmbed({}, '{}', { startupTimeoutMs: 10 })).destroy, 'function')
})

test('zero disables GPU timeout', async t => {
  const f = await fixture(t)
  assert.equal(typeof (await f.module.startEmbed({ late: 25 }, '{}', { startupTimeoutMs: 0 })).destroy, 'function')
})

test('GPU timeout defaults to twenty seconds', async t => {
  const f = await fixture(t)
  const original = globalThis.setTimeout
  const delays = []
  globalThis.setTimeout = (callback, delay, ...args) => {
    delays.push(delay)
    return original(callback, delay, ...args)
  }
  t.after(() => { globalThis.setTimeout = original })
  const handle = await f.module.startEmbed({}, '{}')
  handle.destroy()
  assert.equal(delays.includes(20_000), true)
})

test('timeout removes the device-loss listener', async t => {
  const f = await fixture(t)
  let calls = 0
  await f.module.startEmbed({ never: true }, '{}', { startupTimeoutMs: 5, onDeviceLost() { calls++ } }).catch(() => {})
  globalThis.dispatchEvent(new Event('qni-device-lost'))
  assert.equal(calls, 0)
})

test('startup failures are QniStartupError instances', async t => {
  const f = await fixture(t)
  assert.equal((await f.module.startEmbed({ fail: true }, '{}').catch(e => e)) instanceof f.module.QniStartupError, true)
})

test('never-settling adapter probe times out', { timeout: 1000 }, async t => {
  const f = await fixture(t, undefined, false, { gpu: 'never' })
  assert.equal((await f.module.startEmbed({}, '{}', { startupTimeoutMs: 5 }).catch(e => e)).code, 'startup-timeout')
})

test('early preparation without WebAssembly is classified', async t => {
  const f = await fixture(t, undefined, false, { noWasm: true })
  assert.equal((await f.module.prepareEmbed().catch(e => e)).code, 'wasm-unavailable')
})

test('onDeviceLost receives the stable loss code', async t => {
  const f = await fixture(t)
  let received
  const handle = await f.module.startEmbed({}, '{}', { onDeviceLost: error => { received = error } })
  globalThis.dispatchEvent(new Event('qni-device-lost'))
  handle.destroy()
  assert.equal(received.code, 'gpu-device-lost')
})

test('post-start device loss message does not describe startup', async t => {
  const f = await fixture(t)
  let received
  const handle = await f.module.startEmbed({}, '{}', { onDeviceLost: e => { received = e } })
  globalThis.dispatchEvent(new Event('qni-device-lost'))
  handle.destroy()
  assert.doesNotMatch(received.message, /during startup/)
})
