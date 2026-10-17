// Independent of wasm and the application loader; safe to import before capability checks.
export const STARTUP_ERROR_CODES = Object.freeze([
  'insecure-context', 'wasm-unavailable', 'webgpu-unsupported', 'gpu-adapter-unavailable',
  'gpu-device-unavailable', 'gpu-device-lost', 'load-failed', 'startup-timeout', 'invalid-embed', 'unknown',
])

export class QniStartupError extends Error {
  constructor(code, message, { cause, detail } = {}) {
    super(message, cause === undefined ? undefined : { cause })
    this.name = 'QniStartupError'
    this.code = STARTUP_ERROR_CODES.includes(code) ? code : 'unknown'
    this.detail = detail ?? {}
  }
}

function messageOf(error) {
  try { return typeof error?.message === 'string' ? error.message : String(error) }
  catch { return 'Unknown error' }
}

export function classifyStartupError(error) {
  if (STARTUP_ERROR_CODES.includes(error?.code)) {
    if (error.name === 'QniStartupError') return error
    return new QniStartupError(error.code, messageOf(error), { cause: error, detail: error.detail })
  }
  if (error instanceof TypeError && /dynamically imported module|module script|Failed to fetch|NetworkError|Load failed/i.test(error.message)) {
    return new QniStartupError('load-failed', error.message, { cause: error })
  }
  return new QniStartupError('unknown', messageOf(error), { cause: error })
}

export function preflightSync(env = globalThis) {
  if (env.isSecureContext === false) return new QniStartupError('insecure-context', `Insecure context: ${env.location?.protocol ?? '?'}`)
  if (typeof env.WebAssembly !== 'object' || typeof env.WebAssembly?.instantiate !== 'function') return new QniStartupError('wasm-unavailable', 'WebAssembly is not available')
  if (typeof env.navigator?.gpu?.requestAdapter !== 'function') return new QniStartupError('webgpu-unsupported', 'navigator.gpu is not available')
  return null
}

// Only pending probes are shared, and never across injected environments.
const probes = new WeakMap()
const adapterInfo = new WeakMap()
export async function diagnoseWebGpu(env = globalThis) {
  const early = preflightSync(env)
  if (early) return early
  if (!probes.has(env)) {
    adapterInfo.delete(env)
    const probe = Promise.resolve().then(async () => {
      try {
        const adapter = await env.navigator.gpu.requestAdapter({ powerPreference: 'high-performance' })
        if (!adapter) return new QniStartupError('gpu-adapter-unavailable', 'navigator.gpu.requestAdapter() returned null', { detail: { reason: 'null' } })
        try {
          const info = adapter.info
          adapterInfo.set(env, info ? [info.vendor, info.architecture, info.device, info.description].filter(Boolean).join(' / ') : '')
        } catch { /* Adapter metadata is optional. */ }
        return null
      } catch (cause) {
        return new QniStartupError('gpu-adapter-unavailable', `navigator.gpu.requestAdapter() threw: ${messageOf(cause)}`, { cause, detail: { reason: 'threw' } })
      }
    }).finally(() => probes.delete(env))
    probes.set(env, probe)
  }
  return probes.get(env)
}

export function detectPlatform(nav = globalThis.navigator ?? {}) {
  const ua = nav.userAgent ?? ''
  const hint = nav.userAgentData?.platform ?? ''
  const source = hint || ua
  let os = 'other'
  if (/iPhone|iPod/i.test(source) || /^iOS$/i.test(hint)) os = 'ios'
  else if (/iPad/i.test(source) || (/Mac/i.test(source) && nav.maxTouchPoints > 1)) os = 'ipados'
  else if (/Android/i.test(source)) os = 'android'
  else if (/CrOS|Chrome OS/i.test(source)) os = 'chromeos'
  else if (/Windows/i.test(source)) os = 'windows'
  else if (/Mac/i.test(source)) os = 'macos'
  else if (/Linux/i.test(source)) os = 'linux'
  let browser = 'other', version = null
  for (const [name, re] of [['edge', /Edg[A-Z]?\/(\d+)/], ['firefox', /(?:Firefox|FxiOS)\/(\d+)/], ['samsung', /SamsungBrowser\/(\d+)/], ['chrome', /(?:CriOS|Chrome)\/(\d+)/], ['safari', /Version\/(\d+).*Safari/]]) {
    const match = ua.match(re)
    if (match) { browser = name; version = Number(match[1]); break }
  }
  return { os, browser, version, mobile: ['ios', 'ipados', 'android'].includes(os) || nav.userAgentData?.mobile === true }
}

const browserNames = { chrome: 'Chrome', edge: 'Edge', firefox: 'Firefox', safari: 'Safari', samsung: 'Samsung Internet' }
const osNames = { ios: 'iPhone', ipados: 'iPad', android: 'Android', windows: 'Windows', macos: 'macOS', chromeos: 'ChromeOS', linux: 'Linux' }
function unsupportedSteps({ os, browser, version }) {
  const old = n => version !== null && version < n
  if (os === 'ios' || os === 'ipados') return [`「設定」>「一般」>「ソフトウェアアップデート」で、${os === 'ipados' ? 'iPadOS 26' : 'iOS 26'} 以降に更新してください。`, '更新したら Safari で開いてください。OS の更新が必要な場合があります。']
  if (os === 'android') {
    if (browser === 'firefox') return ['Firefox の Android 版はまだ WebGPU に対応していません。Google Chrome の最新版で開いてください。']
    if (browser === 'chrome' && old(121)) return ['Google Play で Chrome を最新版に更新してください。']
    return ['Google Chrome の最新版で開いてください (Android 12 以降が必要です)。', '古い端末や一部の GPU では動きません。そのときはパソコンで開いてください。']
  }
  if (os === 'windows' && browser === 'firefox' && old(141)) return ['Firefox を最新版に更新するか、Google Chrome か Microsoft Edge で開いてください。']
  if (os === 'macos' && browser === 'safari' && old(26)) return ['macOS を Tahoe 26 以降に更新してください (Safari 26 が入ります)。', '更新できない場合は、Google Chrome の最新版で開いてください。']
  if (os === 'macos' && browser === 'firefox' && old(147)) return ['Firefox を最新版に更新するか、Google Chrome か Safari で開いてください。']
  if (os === 'linux' && browser === 'firefox') return ['Linux 版の Firefox はまだ WebGPU に対応していません。Google Chrome の最新版で開いてください。ただし GPU の種類によっては動きません。']
  if (['chrome', 'edge'].includes(browser) && old(113)) return [`${browserNames[browser]} が古いようです (バージョン ${version})。最新版に更新してください。`]
  if (os === 'linux') return ['Google Chrome の最新版で開いてください。Linux では、GPU の種類によっては Chrome でもまだ動きません。そのときは Windows、Mac、Chromebook で開いてください。']
  if (['windows', 'macos', 'chromeos'].includes(os)) return ['Google Chrome か Microsoft Edge の最新版で開いてください (Mac は Safari 26 以降も可)。', 'すでに最新版なら、管理者の設定や拡張機能で止められている可能性があります。']
  return ['パソコンなら Chrome か Edge の最新版、iPhone・iPad なら iOS 26 以降の Safari、Android なら Chrome で開いてください。']
}

function adapterSteps({ os, browser }) {
  if (['ios', 'ipados'].includes(os)) return ['iOS / iPadOS を最新版に更新してください。', '直らなければ、別の端末で試してください。']
  if (os === 'android') return ['Chrome と Android を最新版に更新してください。', 'この端末の GPU が WebGPU に対応していない可能性があります。そのときはパソコンで開いてください。']
  if (browser === 'safari' && os === 'macos') return ['macOS を最新版に更新してください。', '直らなければ Google Chrome で試してください。']
  return ['ブラウザーの設定で「ハードウェア アクセラレーション」(グラフィック アクセラレーション) をオンにして、ブラウザーを再起動してください。', 'ブラウザーと OS を最新版に更新してください。', '直らなければ、別のブラウザーや端末で試してください。']
}

// Content is deterministic for its arguments; browser metadata is appended only by the renderer.
export function startupErrorContent(error, platform = detectPlatform()) {
  const e = classifyStartupError(error), d = e.detail ?? {}
  const c = { code: e.code, motif: 'crayon', title: '量子回路を起動できませんでした', cause: '予想していなかったエラーが起きました。', environment: '', steps: ['「再試行」を押してください。', '直らなければ、ページを再読み込みするか、別のブラウザーで開いてください。', 'それでも直らないときは、「エラーの詳細」をコピーしてページの作成者に知らせてください。'], primary: 'retry', secondary: null, details: `code: ${e.code}\nmessage: ${messageOf(e)}`, detailsOpen: false }
  const environment = [osNames[platform.os], browserNames[platform.browser] && `${browserNames[platform.browser]}${platform.version == null ? '' : ` ${platform.version}`}`].filter(Boolean).join(' ・ ')
  if (environment) c.environment = `お使いの環境: ${environment}`
  switch (e.code) {
    case 'insecure-context': Object.assign(c, { motif: 'lock', title: '安全な接続で開いてください', cause: 'このページは「http://」で開かれています。安全でない接続では、ブラウザーが GPU を使わせてくれません。', steps: ['アドレスバーの URL を「https://」で始まるものに変えて、開き直してください。', '先生やページの管理者から教わった URL が正しいか、もう一度確かめてください。'], primary: null, advanced: { summary: '詳しい人向け', items: ['WebGPU は安全なコンテキスト (https、または localhost) でしか使えません。'] } }); break
    case 'wasm-unavailable': Object.assign(c, { motif: 'shield', title: 'ブラウザーの設定で止められています', cause: '量子回路の計算に必要な「WebAssembly」という機能が、このブラウザーでは使えません。セキュリティを強める設定でオフになっているか、ブラウザーがとても古い可能性があります。', steps: ['iPhone・iPad・Mac で「ロックダウンモード」をオンにしている場合は、このサイトを除外するか、ロックダウンモードを使っていない端末で開いてください。', 'セキュリティ関係の拡張機能や、ブラウザーの「セキュリティ強化」の設定を確かめてください。', 'ブラウザーを最新版に更新してください。'], primary: null }); break
    case 'webgpu-unsupported': Object.assign(c, { motif: 'browser', title: 'このブラウザーでは動きません', cause: '量子回路の計算には「WebGPU」という、ブラウザーから GPU (画像や計算を速く処理する部品) を使う機能が必要です。このブラウザーはまだ WebGPU に対応していないか、オフになっています。', steps: unsupportedSteps(platform), note: '学校や会社の端末では、管理者が機能を止めていることがあります。そのときは先生や管理者に相談してください。', primary: null, secondary: 'copy-url' }); break
    case 'gpu-adapter-unavailable': {
      Object.assign(c, { motif: 'chip', title: 'GPU を使えませんでした', cause: 'ブラウザーは WebGPU に対応していますが、この端末の GPU を使えませんでした。設定で GPU がオフになっているか、GPU やドライバー (GPU を動かすソフト) が古いか、対応していない可能性があります。', steps: adapterSteps(platform), note: 'リモートデスクトップや仮想マシン、一部の学校の端末では GPU を使えないことがあります。' })
      if (!platform.mobile && ['chrome', 'edge', 'firefox'].includes(platform.browser)) {
        const address = { chrome: 'chrome://gpu', edge: 'edge://gpu', firefox: 'about:support' }[platform.browser]
        c.advanced = { summary: '詳しい人向け', items: [`アドレスバーに ${address} を入力すると、GPU の状態を確かめられます。`], address }
        if (platform.os === 'linux' && platform.browser === 'chrome') c.advanced.items.push('GPU によっては chrome://flags/#enable-unsafe-webgpu を有効にすると動くことがありますが、ブラウザーの安全性が下がります。自分の端末で Qni を使うときだけにし、終わったら元に戻してください。')
      }
      break
    }
    case 'gpu-device-unavailable': Object.assign(c, { motif: 'chip', title: 'GPU の準備に失敗しました', cause: 'GPU は見つかりましたが、計算に使う準備ができませんでした。ほかのアプリやタブが GPU をたくさん使っているか、この GPU では性能が足りない可能性があります。', steps: ['「再試行」を押してください。', '直らなければ、ほかのタブやアプリ (ゲーム・動画編集など) を閉じてから、ブラウザーを再起動してください。', 'OS とブラウザーを最新版に更新してください。GPU のドライバーの更新も確認してください。'] }); break
    case 'gpu-device-lost': Object.assign(c, { motif: 'unplug', title: 'GPU との接続が切れました', cause: '計算の途中で GPU が使えなくなりました。スリープからの復帰、ドライバーの更新、メモリー不足などで起きることがあります。', steps: ['「再試行」を押してください。回路は最初の状態に戻ります。', '何度も起きるときは、ほかのタブを閉じてからページを再読み込みしてください。'] }); break
    case 'load-failed': Object.assign(c, { motif: 'cloud', title: '読み込みに失敗しました', cause: d.offline === true ? 'この端末はインターネットにつながっていないようです。' : Number.isInteger(d.status) && d.status >= 400 && d.status <= 599 ? `サーバーからファイルを受け取れませんでした (エラー ${d.status})。` : '量子回路を動かすプログラムをダウンロードできませんでした。インターネットの接続が切れているか、不安定な可能性があります。', steps: ['インターネットにつながっているか確かめて、「再試行」を押してください。', '直らなければ、ページを再読み込みしてください。', '学校や会社のネットワークでは、ファイルのダウンロードが止められていることがあります。先生や管理者に相談してください。'] }); break
    case 'startup-timeout': Object.assign(c, { motif: 'chip', title: '起動が止まってしまいました', cause: `GPU の準備が ${Number.isFinite(d.timeoutMs) && d.timeoutMs > 0 ? d.timeoutMs / 1000 : 20} 秒たっても終わりませんでした。ブラウザーの GPU の処理が、一時的に止まっている可能性があります。`, steps: ['「ページを再読み込み」を押してください。', '直らなければ、ブラウザーをいったん終了して開き直してください。'], primary: 'reload' }); break
    case 'invalid-embed': Object.assign(c, { motif: 'circuit', title: '回路のデータに問題があります', cause: 'このページに書かれている量子回路の設定が正しくありません。あなたの操作やブラウザーのせいではありません。', steps: ['ページの作成者に知らせてください。下の「エラーの詳細」をコピーして送ると、原因が早く分かります。'], primary: null, detailsOpen: true }); break
  }
  return c
}

// Colors: src/colors/flexoki.rs and kepano/flexoki css/flexoki.css.
// Hover uses the TX overlay instead of the design's unverified blue-700.
const CSS = `
:host{--bg:#FFFCF0;--bg-2:#F2F0E5;--ui:#E6E4D9;--ui-3:#CECDC3;--tx-3:#B7B5AC;--tx-2:#6F6E69;--tx:#100F0F;--blue-600:#205EA6;--blue-400:#4385BE;--blue-200:#92BFDB;--purple:#5E409D;--purple-bg:#F0EAEC;--yellow:#D0A215;--orange:#DA702C;--red:#AF3029;display:block;height:100%;min-width:0;color-scheme:light;color:var(--tx);background:var(--bg);font:14px/20px var(--qni-startup-error-font,system-ui,-apple-system,"Segoe UI","Hiragino Sans","Noto Sans JP","Yu Gothic UI",Meiryo,sans-serif);text-align:left;letter-spacing:normal;container:qse / inline-size}
*,*::before,*::after{box-sizing:border-box} .scroller{color:var(--tx);font:14px/20px var(--qni-startup-error-font,system-ui,-apple-system,"Segoe UI","Hiragino Sans","Noto Sans JP","Yu Gothic UI",Meiryo,sans-serif);text-align:left;letter-spacing:normal;height:100%;overflow:auto;overscroll-behavior:contain;display:flex;flex-direction:column;padding:16px}.panel{display:grid;grid-template-columns:64px minmax(0,1fr);gap:12px;max-width:768px;width:100%;margin:auto;flex-shrink:0}.art{width:64px;height:48px;align-self:center}.art svg{width:100%;height:100%;overflow:visible}.copy{display:contents}h2,h3,h4{font-size:16px;line-height:24px;margin:0;font-weight:700;align-self:center}p,ol,details,.actions{grid-column:1 / -1;margin:0}p,li{overflow-wrap:anywhere}.cause,.note,.env{color:var(--tx-2)}.label,.env,details{font-size:12px;line-height:16px}.label{font-weight:700}.steps{padding:0;list-style:none;counter-reset:step;display:grid;gap:8px}.steps li{counter-increment:step;position:relative;padding-left:28px}.steps li::before{content:counter(step);position:absolute;left:0;top:0;width:20px;height:20px;border-radius:50%;background:var(--ui);text-align:center;font-size:12px;line-height:20px}.actions{display:flex;flex-wrap:wrap;gap:8px}button{font:inherit;line-height:20px;color:var(--tx);background:var(--bg);border:1px solid var(--ui-3);border-radius:6px;min-height:40px;padding:8px 16px;cursor:pointer;white-space:normal}button.primary{color:var(--bg);background:var(--blue-600);border-color:var(--blue-600);width:100%}button:hover{background:var(--ui)}button.primary:hover{background:var(--blue-600);box-shadow:inset 0 0 0 999px rgb(16 15 15 / .1)}button:active{transform:translateY(1px)}button:disabled{opacity:.6;cursor:wait}button:focus-visible,summary:focus-visible{outline:2px solid var(--blue-600);outline-offset:2px}summary{cursor:pointer;padding:8px 0}details{min-width:0}details p{margin:8px 0}.advanced{background:var(--purple-bg);padding:0 12px;border-radius:6px}.raw pre{background:var(--bg-2);padding:12px;max-height:160px;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;user-select:text;font:12px/16px ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}.advanced code{overflow-wrap:anywhere;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}.sr-only{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;border:0}
.ink{fill:none;stroke-width:4;stroke-linecap:round;stroke-linejoin:round;mix-blend-mode:multiply}.pencil{stroke:var(--tx-3);stroke-width:2;stroke-dasharray:3 5}.blue{stroke:var(--blue-600)}.light{stroke:var(--blue-200)}.cloud{stroke:var(--blue-400)}.purple{stroke:var(--purple)}.yellow{stroke:var(--yellow)}.orange{stroke:var(--orange)}.gray{stroke:var(--tx-2)}.mark{stroke:var(--red)}.w3{stroke-width:3}.draw{stroke-dasharray:100;stroke-dashoffset:0;animation:draw 600ms cubic-bezier(.55,0,.9,.5) 150ms both}.mark{animation:mark 120ms 750ms both}.cause{animation:rise 400ms both}@keyframes draw{from{stroke-dashoffset:100}to{stroke-dashoffset:0}}@keyframes mark{from{opacity:0}to{opacity:1}}@keyframes rise{from{opacity:0;transform:translateY(12px)}to{opacity:1;transform:translateY(0)}}
@container qse (min-width:560px){.scroller{padding:24px}.panel{grid-template-columns:160px minmax(0,576px);gap:24px}.art{width:160px;height:120px;align-self:start}.copy{display:flex;flex-direction:column;gap:12px;min-width:0}h2,h3,h4{font-size:18px;line-height:28px;align-self:auto}button.primary{width:auto}}
@media(pointer:coarse){button{min-height:44px}}@media(prefers-reduced-motion:reduce){*,*::before,*::after{animation:none!important;transition:none!important}}
`

// Trusted static SVG only. No user, UA or error text enters these fragments.
function illustration(motif, timeout) {
  const chip = '<rect class="blue" x="110" y="38" width="40" height="44" rx="6"/><path class="blue w3" d="M118 32V38 M130 32V38 M142 32V38 M118 82V88 M130 82V88 M142 82V88 M104 50H110 M104 70H110"/>'
  const shapes = {
    chip: chip + (timeout ? '<path class="purple w3" d="M118 12H142 M118 30H142 M122 12C122 20 138 22 138 30 M138 12C138 20 122 22 122 30"/>' : ''),
    browser: '<rect class="gray" x="80" y="20" width="74" height="80" rx="6"/><path class="gray w3" d="M80 36H154 M88 28H90 M98 28H100"/><rect class="pencil" x="116" y="56" width="28" height="28"/>',
    lock: '<rect class="yellow" x="108" y="52" width="42" height="40" rx="6"/><path class="yellow" d="M118 52V30C118 12 144 12 144 30 M130 66V78"/>',
    shield: '<path class="purple" d="M126 20L152 32V58Q152 82 126 98Q100 82 100 58V32Z"/>',
    unplug: '<rect class="blue" x="122" y="36" width="30" height="44" rx="4"/><path class="blue" d="M80 90Q104 100 106 76 M98 60H114V74H98Z M102 60V50 M110 60V50"/><path class="orange w3" d="M98 38L94 30 M108 36V26 M118 40L124 34"/>',
    cloud: '<path class="cloud" d="M104 58H94C74 58 78 34 94 34C94 10 126 10 130 30C154 24 162 58 140 58H132"/><path class="blue" d="M120 44V68 M120 84V100 M112 92L120 100L128 92"/>',
    circuit: '<path class="pencil" d="M80 42H154 M80 84H154"/><rect class="purple" x="106" y="26" width="36" height="32" rx="4"/><path class="purple w3" d="M119 35C119 29 132 30 132 36C132 41 125 40 125 46 M125 51V52"/>',
    crayon: '<path class="blue" d="M88 82L104 94L124 68L108 56Z M116 46L132 58L148 38L142 22L128 28Z"/><path class="light" d="M108 56L124 68 M116 46L132 58"/>',
  }
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 120" aria-hidden="true" focusable="false"><defs><filter id="crayon" x="-10%" y="-10%" width="120%" height="120%"><feTurbulence type="fractalNoise" baseFrequency="0.05" numOctaves="1" seed="2" result="n"/><feDisplacementMap in="SourceGraphic" in2="n" scale="1.5" xChannelSelector="R" yChannelSelector="G"/></filter></defs><g class="ink" filter="url(#crayon)"><path class="pencil" d="M8 60H76"/><rect class="orange" x="28" y="46" width="28" height="28" rx="4"/><path class="orange w3" d="M36 53V67 M48 53V67 M36 60H48"/><path class="blue draw" pathLength="100" d="M56 60C64 58 70 62 78 60"/>${shapes[motif] ?? shapes.crayon}<path class="mark w3" d="M84 53L92 67 M92 53L84 67"/></g></svg>`
}

/** Replace a container's contents with an isolated, accessible startup error view. */
export function renderStartupError(container, error, { onRetry, platform, headingLevel = 3 } = {}) {
  const doc = container.ownerDocument
  const win = doc.defaultView
  const c = startupErrorContent(error, platform ?? detectPlatform(win?.navigator ?? {}))
  if (c.primary === 'retry' && typeof onRetry !== 'function') {
    c.steps = c.steps.map(step => step.replaceAll('「再試行」', '「ページを再読み込み」'))
  }
  // Walk active elements, rather than assuming the container is in the document light DOM.
  let active = doc.activeElement
  while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement
  let ancestor = active, restoreFocus = false
  while (ancestor) {
    if (ancestor === container) { restoreFocus = true; break }
    ancestor = ancestor.parentNode ?? ancestor.host
  }
  const host = doc.createElement('div')
  host.dataset.qniStartupError = c.code
  const root = host.attachShadow({ mode: 'open' })
  function el(tag, className, text, parent) {
    const node = doc.createElement(tag)
    if (className) node.className = className
    if (text !== undefined) node.textContent = text
    if (parent) parent.append(node)
    return node
  }
  el('style', '', CSS, root)
  const scroller = el('div', 'scroller', undefined, root)
  const panel = el('section', 'panel', undefined, scroller)
  panel.setAttribute('aria-labelledby', 'title')
  panel.setAttribute('aria-describedby', 'cause')
  const art = el('div', 'art', undefined, panel)
  art.innerHTML = illustration(c.motif, c.code === 'startup-timeout')
  const copy = el('div', 'copy', undefined, panel)
  const title = el(`h${[2, 3, 4].includes(headingLevel) ? headingLevel : 3}`, '', c.title, copy)
  title.id = 'title'
  title.tabIndex = -1
  el('p', 'cause', c.cause, copy).id = 'cause'
  if (c.environment) el('p', 'env', c.environment, copy)
  el('p', 'label', 'やってみること', copy)
  const steps = el('ol', 'steps', undefined, copy)
  for (const step of c.steps) el('li', '', step, steps)
  if (c.note) el('p', 'note', c.note, copy)
  const status = el('p', 'sr-only', '', root)
  status.setAttribute('role', 'status')
  let disposed = false
  const timers = new Set()
  function later(fn, ms) {
    const timer = setTimeout(() => { timers.delete(timer); if (!disposed && host.parentNode === container) fn() }, ms)
    timers.add(timer)
  }
  function button(label, parent, handler, className = '') {
    const b = el('button', className, label, parent)
    b.type = 'button'
    b.addEventListener('click', handler)
    return b
  }
  function details(className, label, open = false) {
    const node = el('details', className, undefined, copy)
    node.open = open
    el('summary', '', label, node)
    return node
  }
  function selectText(node) {
    try {
      const range = doc.createRange()
      range.selectNodeContents(node)
      const selection = root.getSelection?.() ?? win?.getSelection()
      selection?.removeAllRanges()
      selection?.addRange(range)
    } catch { /* Selection may be denied by the embedding environment. */ }
  }
  function copyButton(label, text, parent, fallback) {
    const b = button(label, parent, async () => {
      b.disabled = true
      let result
      try {
        if (!win?.navigator?.clipboard?.writeText) throw new Error('Clipboard unavailable')
        await win.navigator.clipboard.writeText(text)
        result = 'コピーしました'
      } catch {
        result = 'コピーできませんでした'
        if (!disposed && host.parentNode === container) {
          if (!fallback) raw.open = true
          if (fallback) fallback.hidden = false
          selectText(fallback ?? rawText)
        }
      }
      if (disposed || host.parentNode !== container) return
      b.textContent = result
      status.textContent = result
      b.disabled = false
      later(() => { b.textContent = label }, 2000)
    })
    return b
  }
  if (c.primary || c.secondary) {
    const actions = el('div', 'actions', undefined, copy)
    if (c.primary) {
      const retry = c.primary === 'retry' && typeof onRetry === 'function'
      const b = button(retry ? '再試行' : 'ページを再読み込み', actions, () => {
        b.disabled = true
        if (retry) onRetry()
        else win?.location.reload()
      }, 'primary')
      b.dataset.action = retry ? 'retry' : 'reload'
    }
    if (c.secondary) {
      const url = el('code', '', doc.URL, actions)
      url.hidden = true
      copyButton('このページの URL をコピー', doc.URL, actions, url)
    }
  }
  if (c.advanced) {
    const advanced = details('advanced', c.advanced.summary)
    for (const item of c.advanced.items) el('p', '', item, advanced)
    if (c.advanced.address) {
      const code = el('code', '', c.advanced.address, advanced)
      copyButton('コピー', c.advanced.address, advanced, code)
    }
  }
  const raw = details('raw', 'エラーの詳細', c.detailsOpen)
  const env = win ?? globalThis
  const rawText = el('pre', '', `${c.details}\nbrowser: ${env.navigator?.userAgent ?? '(なし)'}\nadapter: ${adapterInfo.get(env) || '(なし)'}\nsecure: ${env.isSecureContext === false ? 'no' : 'yes'}`, raw)
  copyButton('詳細をコピー', rawText.textContent, raw, rawText).dataset.copyDetails = ''
  container.replaceChildren(host)
  if (restoreFocus) title.focus({ preventScroll: true })
  const announce = () => {
    if (!disposed && host.parentNode === container) status.textContent = `${c.title}。${c.cause}`
  }
  const frame = win?.requestAnimationFrame ? win.requestAnimationFrame(announce) : null
  if (frame === null) later(announce, 0)
  return {
    code: c.code,
    naturalHeight() {
      const style = win?.getComputedStyle(scroller)
      return panel.scrollHeight + (parseFloat(style?.paddingTop) || 0) + (parseFloat(style?.paddingBottom) || 0)
    },
    remove() {
      disposed = true
      if (frame !== null) win.cancelAnimationFrame(frame)
      for (const timer of timers) clearTimeout(timer)
      timers.clear()
      host.remove()
    },
  }
}
