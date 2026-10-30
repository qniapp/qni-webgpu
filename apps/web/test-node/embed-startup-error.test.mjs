import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { STARTUP_ERROR_CODES, QniStartupError, classifyStartupError, preflightSync, diagnoseWebGpu, detectPlatform, startupErrorContent } from '../qni-embed-error.mjs'

const env = (requestAdapter = async () => ({})) => ({ WebAssembly, navigator: { gpu: { requestAdapter } } })
const platform = (os = 'windows', browser = 'chrome', version = 140) => ({ os, browser, version, mobile: ['ios', 'ipados', 'android'].includes(os) })

test('stable codes are frozen', () => assert.equal(Object.isFrozen(STARTUP_ERROR_CODES), true))
test('unknown constructor code is normalized', () => assert.equal(new QniStartupError('other', 'message').code, 'unknown'))
test('classification preserves same module error', () => { const e = new QniStartupError('unknown', 'message'); assert.equal(classifyStartupError(e), e) })
test('classification preserves another module error', () => { const e = { name: 'QniStartupError', code: 'invalid-embed', message: 'bad', detail: {} }; assert.equal(classifyStartupError(e), e) })
for (const code of ['gpu-adapter-unavailable', 'gpu-device-unavailable', 'invalid-embed']) {
  test(`Rust ${code} retains message`, () => assert.equal(classifyStartupError({ code, message: 'invalid circuit JSON' }).message, 'invalid circuit JSON'))
}
for (const message of ['Failed to fetch dynamically imported module', 'error loading dynamically imported module', 'Importing a module script failed.', 'Load failed', 'NetworkError']) {
  test(`import failure: ${message}`, () => assert.equal(classifyStartupError(new TypeError(message)).code, 'load-failed'))
}
for (const value of [null, undefined, 3, 'oops', { code: 'future' }, new Error('Failed to fetch'), new TypeError('unrelated')]) {
  test(`unrecognized error: ${String(value)}`, () => assert.equal(classifyStartupError(value).code, 'unknown'))
}
test('insecure context takes precedence', () => assert.equal(preflightSync({ isSecureContext: false }).code, 'insecure-context'))
test('WebAssembly absence precedes GPU absence', () => assert.equal(preflightSync({}).code, 'wasm-unavailable'))
test('missing instantiate is unavailable', () => assert.equal(preflightSync({ WebAssembly: {} }).code, 'wasm-unavailable'))
test('missing GPU is unsupported', () => assert.equal(preflightSync({ WebAssembly }).code, 'webgpu-unsupported'))
test('missing requestAdapter is unsupported', () => assert.equal(preflightSync({ WebAssembly, navigator: { gpu: {} } }).code, 'webgpu-unsupported'))
test('undefined secure context passes', () => assert.equal(preflightSync(env()), null))
test('diagnostics accept an adapter', async () => assert.equal(await diagnoseWebGpu(env()), null))
test('null adapter records reason', async () => assert.equal((await diagnoseWebGpu(env(async () => null))).detail.reason, 'null'))
test('throwing adapter preserves cause', async () => { const thrown = new Error('denied'); assert.equal((await diagnoseWebGpu(env(() => { throw thrown }))).cause, thrown) })
test('adapter uses high performance', async () => { let options; await diagnoseWebGpu(env(async o => { options = o; return {} })); assert.deepEqual(options, { powerPreference: 'high-performance' }) })
test('concurrent diagnostics share an inflight probe', async () => { let count = 0; const e = env(async () => { count++; return {} }); await Promise.all([diagnoseWebGpu(e), diagnoseWebGpu(e)]); assert.equal(count, 1) })
test('settled successful probes are not cached', async () => { let count = 0; const e = env(async () => { count++; return {} }); await diagnoseWebGpu(e); await diagnoseWebGpu(e); assert.equal(count, 2) })
test('settled failed probes are not cached', async () => { let count = 0; const e = env(async () => { count++; return null }); await diagnoseWebGpu(e); await diagnoseWebGpu(e); assert.equal(count, 2) })
test('different environments never share probes', async () => assert.deepEqual((await Promise.all([diagnoseWebGpu(env(async () => null)), diagnoseWebGpu(env())])).map(e => e?.code ?? null), ['gpu-adapter-unavailable', null]))

const platforms = [
  ['iPhone Version/26.0 Mobile Safari/605', {}, platform('ios', 'safari', 26)],
  ['Macintosh Version/26.0 Safari/605', { maxTouchPoints: 5 }, platform('ipados', 'safari', 26)],
  ['Android Chrome/120.0 Mobile Safari/537', {}, platform('android', 'chrome', 120)],
  ['Android Firefox/140.0', {}, platform('android', 'firefox', 140)],
  ['Windows NT 10.0 Chrome/140.0 Edg/140.0', {}, platform('windows', 'edge', 140)],
  ['Macintosh Version/18.0 Safari/605', {}, platform('macos', 'safari', 18)],
  ['Linux Firefox/140.0', {}, platform('linux', 'firefox', 140)],
  ['CrOS x86_64 Chrome/140.0', {}, platform('chromeos', 'chrome', 140)],
  ['Android Chrome/140 SamsungBrowser/28', {}, platform('android', 'samsung', 28)],
  ['iPhone CriOS/140.0 Safari/605', {}, platform('ios', 'chrome', 140)],
  ['iPhone FxiOS/140.0 Safari/605', {}, platform('ios', 'firefox', 140)],
  ['iPad CriOS/140.0 Mobile Safari/605', {}, platform('ipados', 'chrome', 140)],
  ['iPad FxiOS/140.0 Mobile Safari/605', {}, platform('ipados', 'firefox', 140)],
  ['', { userAgentData: { platform: 'iOS' } }, platform('ios', 'other', null)],
  ['', {}, platform('other', 'other', null)],
  ['Linux Chrome/140.0', { userAgentData: { platform: 'Windows' } }, platform('windows', 'chrome', 140)],
]
for (const [userAgent, extra, expected] of platforms) test(`platform ${userAgent || 'empty'} ${JSON.stringify(extra)}`, () => assert.deepEqual(detectPlatform({ userAgent, ...extra }), expected))
for (const code of STARTUP_ERROR_CODES) {
  const content = () => startupErrorContent(new QniStartupError(code, 'raw'), platform())
  test(`${code} content retains code`, () => assert.equal(content().code, code))
  test(`${code} content has Japanese title`, () => assert.match(content().title, /[ぁ-んァ-ン一-龠]/u))
  test(`${code} content has cause`, () => assert.equal(Boolean(content().cause), true))
  test(`${code} content has steps`, () => assert.equal(content().steps.length > 0, true))
  test(`${code} content has motif`, () => assert.equal(Boolean(content().motif), true))
  test(`${code} content details retain message`, () => assert.equal(content().details.includes('raw'), true))
}
const content = (code, p = platform(), detail = {}) => startupErrorContent(new QniStartupError(code, 'raw', { detail }), p)
for (const [p, fragment] of [
  [platform('ios', 'safari', 26), 'iOS 26'], [platform('ipados', 'safari', 26), 'iPadOS 26'],
  [platform('android', 'firefox'), 'Chrome'], [platform('android', 'chrome', 120), 'Google Play'], [platform('android', 'samsung'), 'Android 12'],
  [platform('windows', 'firefox', 140), 'Firefox'], [platform('macos', 'safari', 18), 'Tahoe 26'], [platform('macos', 'firefox', 146), 'Firefox'],
  [platform('linux', 'firefox'), 'Chrome'], [platform('windows', 'chrome', 109), 'バージョン 109'], [platform('windows', 'edge', 109), 'バージョン 109'],
  [platform('linux'), 'GPU'], [platform('chromeos'), '管理者'], [platform('other', 'other', null), 'Safari'],
]) test(`unsupported guidance ${JSON.stringify(p)}`, () => assert.equal(content('webgpu-unsupported', p).steps.join().includes(fragment), true))
for (const p of [platform('windows'), platform('windows', 'edge'), platform('windows', 'firefox'), platform('macos', 'safari'), platform('ios'), platform('android'), platform('other', 'other', null)]) {
  test(`adapter guidance exists ${JSON.stringify(p)}`, () => assert.equal(content('gpu-adapter-unavailable', p).steps.length > 0, true))
  test(`no unsafe flag outside Linux ${JSON.stringify(p)}`, () => assert.equal(JSON.stringify(content('gpu-adapter-unavailable', p)).includes('enable-unsafe-webgpu'), false))
}
test('Linux advanced advice includes unsafe flag', () => assert.equal(JSON.stringify(content('gpu-adapter-unavailable', platform('linux')).advanced).includes('enable-unsafe-webgpu'), true))
test('Linux regular steps exclude unsafe flag', () => assert.equal(content('gpu-adapter-unavailable', platform('linux')).steps.join().includes('enable-unsafe-webgpu'), false))
test('no low power mode claims', () => assert.equal(JSON.stringify(content('gpu-adapter-unavailable', platform('ios'))).includes('低電力'), false))
test('no universal iOS engine claim', () => assert.equal(content('webgpu-unsupported', platform('ios')).steps.join().includes('どのブラウザーも'), false))
for (const code of ['insecure-context', 'wasm-unavailable', 'webgpu-unsupported', 'invalid-embed']) test(`${code} has no retry`, () => assert.equal(content(code).primary, null))
test('timeout reloads', () => assert.equal(content('startup-timeout').primary, 'reload'))
test('invalid embed details are open', () => assert.equal(content('invalid-embed').detailsOpen, true))
test('offline load copy', () => assert.match(content('load-failed', platform(), { offline: true }).cause, /つながっていない/))
test('HTTP load copy', () => assert.match(content('load-failed', platform(), { status: 404 }).cause, /404/))
test('timeout uses supplied duration', () => assert.match(content('startup-timeout', platform(), { timeoutMs: 5000 }).cause, /5 秒/))
test('Rust codes are covered', () => { const path = new URL('../src/startup_error.rs', import.meta.url); assert.deepEqual([...readFileSync(path, 'utf8').matchAll(/=>\s*"([a-z]+(?:-[a-z]+)+)"/g)].map(m => m[1]).filter(c => !STARTUP_ERROR_CODES.includes(c)), []) })
for (const [code, motif, primary, secondary] of [
  ['insecure-context', 'lock', null, null], ['wasm-unavailable', 'shield', null, null],
  ['webgpu-unsupported', 'browser', null, 'copy-url'], ['gpu-adapter-unavailable', 'chip', 'retry', null],
  ['gpu-device-unavailable', 'chip', 'retry', null], ['gpu-device-lost', 'unplug', 'retry', null],
  ['load-failed', 'cloud', 'retry', null], ['startup-timeout', 'chip', 'reload', null],
  ['invalid-embed', 'circuit', null, null], ['unknown', 'crayon', 'retry', null],
]) {
  for (const [property, expected] of [['motif', motif], ['primary', primary], ['secondary', secondary]]) {
    test(`content ${property}: ${code}`, () => assert.equal(content(code)[property], expected))
  }
}
test('all CSS colors are exact Rust or official Flexoki tones', () => {
  const rust = readFileSync(new URL('../src/colors/flexoki.rs', import.meta.url), 'utf8')
  const colors = [...rust.matchAll(/from_rgb\(0x([0-9A-F]{2}), 0x([0-9A-F]{2}), 0x([0-9A-F]{2})\)/g)].map(m => `#${m[1]}${m[2]}${m[3]}`)
  colors.push('#CECDC3', '#DA702C', '#D0A215', '#F0EAEC')
  const source = readFileSync(new URL('../qni-embed-error.mjs', import.meta.url), 'utf8')
  assert.deepEqual([...source.matchAll(/#[0-9A-F]{6}\b/g)].map(m => m[0]).filter(color => !colors.includes(color)), [])
})
test('HTTP success status never becomes an error description', () => assert.equal(content('load-failed', platform(), { status: 200 }).cause.includes('エラー 200'), false))
test('invalid timeout falls back to twenty seconds', () => assert.match(content('startup-timeout', platform(), { timeoutMs: -1 }).cause, /20 秒/))
test('adapter metadata access failure does not reject diagnostics', async () => assert.equal(await diagnoseWebGpu(env(async () => ({ get info() { throw new Error('denied') } }))), null))
