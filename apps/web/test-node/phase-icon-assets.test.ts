export {}

const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const repoRoot = path.resolve(__dirname, '../../..')
const phasePathStart = 'M18.2857 36L29.7143 12M16 24.5714'

test('Phase icon sources use the Qni Φ circle-slash glyph while keeping token P', async () => {
  const [svg, component, generator] = await Promise.all([
    fs.readFile(path.join(repoRoot, 'apps/web/assets/icons/p.svg'), 'utf8'),
    fs.readFile(path.join(repoRoot, 'docs/design-system/components/p-gate.js'), 'utf8'),
    fs.readFile(path.join(repoRoot, 'scripts/extract-gate-svg.py'), 'utf8'),
  ])

  assert.deepEqual({
    svgUsesPhaseGlyph: svg.includes(phasePathStart),
    componentUsesPhaseGlyph: component.includes(phasePathStart),
    generatorKeepsPTokenForPhaseGlyph: /if token == "P":\n\s+return phase_svg\(\)/.test(generator),
  }, {
    svgUsesPhaseGlyph: true,
    componentUsesPhaseGlyph: true,
    generatorKeepsPTokenForPhaseGlyph: true,
  })
})
