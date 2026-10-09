const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const root = path.resolve(__dirname, '..')
const config = fs.readFileSync(path.join(root, 'Trunk.toml'), 'utf8')
const index = fs.readFileSync(path.join(root, 'index.html'), 'utf8')

test('Trunk requires the verified Binaryen version rather than an arbitrary PATH tool', () => {
  const tools = config.split('[tools]')[1]?.split(/^\[/m)[0]
  assert.equal(tools?.match(/^wasm_opt\s*=\s*"([^"]+)"/m)?.[1], 'version_123')
})

test('production optimizer only enables the two verified Rust instruction features', () => {
  assert.deepEqual({
    level: index.match(/data-wasm-opt="([^"]+)"/)?.[1],
    flags: index.match(/data-wasm-opt-params="([^"]+)"/)?.[1],
  }, {
    level: 'z',
    flags: '--enable-bulk-memory --enable-nontrapping-float-to-int',
  })
})

test('size-focused eframe features are limited to wasm while native defaults remain', () => {
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8')
  const features = (condition: string): string[] => {
    const section = cargo.split(`[target.'cfg(${condition})'.dependencies]`)[1]?.split(/^\[/m)[0]
    return section?.match(/features = \[([^\]]+)\]/)?.[1].match(/"([^"]+)"/g)?.map((value: string) => value.slice(1, -1)) ?? []
  }
  assert.deepEqual({
    wasm: features('target_arch = "wasm32"'),
    native: features('not(target_arch = "wasm32")'),
  }, {
    wasm: ['wgpu_no_default_features', 'web_screen_reader'],
    native: ['wgpu', 'default_fonts', 'web_screen_reader'],
  })
})

export {}
