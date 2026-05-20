const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const chanceInfoBlock = (): string => {
  const source = fs.readFileSync(path.resolve(__dirname, '../src/gates/info.rs'), 'utf8')
  const block = source.match(/GateKind::ChanceDisplay => GateInfo \{([\s\S]*?transitions: &\[\],\n\s*)\},/)?.[1]
  if (!block) {
    throw new Error('ChanceDisplay GateInfo block not found')
  }
  return block
}

const chanceParagraphs = (): string[] => {
  const paragraphs = chanceInfoBlock().match(/paragraphs: &\[([\s\S]*?)\],/)?.[1]
  if (!paragraphs) {
    throw new Error('ChanceDisplay paragraphs block not found')
  }
  return [...paragraphs.matchAll(/"([^"]+)"/g)].map((match) => match[1])
}

test('Chance tooltip title matches Quirk probability display title', () => {
  assert.match(chanceInfoBlock(), /name: "Probability Display"/)
})

test('Chance tooltip description matches Quirk probability display blurb', () => {
  assert.deepEqual(chanceParagraphs(), [
    'Shows chances of outcomes if a measurement was performed.',
    'Use controls to see conditional probabilities.',
  ])
})

export {}
