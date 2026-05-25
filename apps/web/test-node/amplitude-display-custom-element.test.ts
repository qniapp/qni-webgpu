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

test('Gate palette spec renders both Amps examples with the amplitude-display custom element', async () => {
  const source = await fs.readFile(paletteSpecPath, 'utf8')

  assert.equal(source.match(/<amplitude-display icon no-popup><\/amplitude-display>/g)?.length, 2)
})

test('Vertical divider spec renders the Amps slot with the amplitude-display custom element', async () => {
  const source = await fs.readFile(verticalDividerSpecPath, 'utf8')

  assert.match(source, /<amplitude-display icon no-popup><\/amplitude-display>/)
})
