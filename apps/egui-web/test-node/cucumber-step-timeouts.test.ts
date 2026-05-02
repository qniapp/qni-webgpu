const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const stepsDir = path.join(__dirname, '..', 'features', 'step_definitions')

const readStep = (name: string) => fs.readFile(path.join(stepsDir, name), 'utf8')

test('standard browser startup steps declare explicit cucumber timeouts', async () => {
  const source = await readStep('startup-success.steps.ts')

  assert.match(
    source,
    /Given\(\s*'the egui web app is open in the standard WebGPU browser',\s*\{\s*timeout:\s*\w+\s*\}/,
  )
  assert.match(
    source,
    /When\('the app finishes initializing',\s*\{\s*timeout:\s*\w+\s*\}/,
  )
})

test('plain chromium startup steps declare explicit cucumber timeouts', async () => {
  const source = await readStep('plain-chromium-error.steps.ts')

  assert.match(
    source,
    /Given\(\s*'the egui web app is open in plain chromium',\s*\{\s*timeout:\s*\w+\s*\}/,
  )
  assert.match(
    source,
    /When\('the plain chromium session finishes loading',\s*\{\s*timeout:\s*\w+\s*\}/,
  )
})

export {}
