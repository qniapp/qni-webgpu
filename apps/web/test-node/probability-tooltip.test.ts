const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const probabilityInfoBlock = (): string => {
  const source = fs.readFileSync(path.resolve(__dirname, '../src/gates/info.rs'), 'utf8')
  const block = source.match(/GateKind::ProbabilityDisplay => GateInfo \{([\s\S]*?transitions: &\[\],\n\s*)\},/)?.[1]
  if (!block) {
    throw new Error('ProbabilityDisplay GateInfo block not found')
  }
  return block
}

const probabilityParagraphs = (): string[] => {
  const paragraphs = probabilityInfoBlock().match(/paragraphs: &\[([\s\S]*?)\],/)?.[1]
  if (!paragraphs) {
    throw new Error('ProbabilityDisplay paragraphs block not found')
  }
  return [...paragraphs.matchAll(/"([^"]+)"/g)].map((match) => match[1])
}

test('Probability tooltip title matches Quirk probability display title', () => {
  assert.match(probabilityInfoBlock(), /name: "Probability Display"/)
})

test('Probability tooltip description matches Quirk probability display blurb', () => {
  assert.deepEqual(probabilityParagraphs(), [
    'Shows probabilities of outcomes if a measurement was performed.',
    'Use controls to see conditional probabilities.',
  ])
})

export {}
