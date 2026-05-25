const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')
const docsRootDir = path.join(rootDir, '..', '..', 'docs', 'design-system')
const componentPath = path.join(docsRootDir, 'components', 'amplitude-display.js')
const amplitudeSpecPath = path.join(docsRootDir, 'amplitude-display.html')
const paletteSpecPath = path.join(docsRootDir, 'gate-palette.html')
const verticalDividerSpecPath = path.join(docsRootDir, 'vertical-divider.html')

export {}

test('Amplitude display design component registers the canonical custom element', async () => {
  const source = await fs.readFile(componentPath, 'utf8')

  assert.match(source, /const ELEMENT_NAME = 'amplitude-display'[\s\S]*customElements\.define\(ELEMENT_NAME, AmplitudeDisplay\)/)
})

test('Amplitude display design component is syntactically valid JavaScript', async () => {
  const source = await fs.readFile(componentPath, 'utf8')

  assert.doesNotThrow(() => new Function(source))
})

test('Amplitude display spec uses the custom element instead of page-local ad-host nodes', async () => {
  const source = await fs.readFile(amplitudeSpecPath, 'utf8')

  assert.doesNotMatch(source, /class="ad-host"/)
})

test('Amplitude display spec explains the complex-amplitude visual encoding before the critique', async () => {
  const source = await fs.readFile(amplitudeSpecPath, 'utf8')
  const sectionIndex = source.indexOf('§ 00 複素振幅の視覚表現選定根拠')
  const critiqueIndex = source.indexOf('批評 ·')

  assert.ok(sectionIndex !== -1 && critiqueIndex !== -1 && sectionIndex < critiqueIndex)
})

test('Amplitude display spec includes a display-block choice guide', async () => {
  const source = await fs.readFile(amplitudeSpecPath, 'utf8')

  assert.match(source, /<h3[^>]*>確率表示ブロックとの使い分け<\/h3>[\s\S]*<table class="spec">[\s\S]*読みたいもの[\s\S]*振幅表示ブロック[\s\S]*確率表示ブロック[\s\S]*密度行列表示ブロック/)
})

test('Amplitude display spec separates production and mock cell sizes', async () => {
  const source = await fs.readFile(amplitudeSpecPath, 'utf8')

  assert.match(source, /本番 cellPx[\s\S]*モック cellPx[\s\S]*本番は span 16 でも 3 px が最小なので heatmap 描画への切替は発生しない[\s\S]*モックでは縮小表示の都合で span 13〜16 だけヒートマップモード/)
})

test('Gate palette spec renders both Amps examples with the amplitude-display custom element', async () => {
  const source = await fs.readFile(paletteSpecPath, 'utf8')

  assert.equal(source.match(/<amplitude-display icon no-popup><\/amplitude-display>/g)?.length, 2)
})

test('Vertical divider spec renders the Amps slot with the amplitude-display custom element', async () => {
  const source = await fs.readFile(verticalDividerSpecPath, 'utf8')

  assert.match(source, /<amplitude-display icon no-popup><\/amplitude-display>/)
})
