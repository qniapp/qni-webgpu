const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const rootDir = path.join(__dirname, '..')

const f32 = (value: number): number => new Float32Array([value])[0]

const extractFunction = (source: string, name: string): string => {
  const start = source.indexOf(`fn ${name}(`)
  if (start < 0) {
    throw new Error(`missing ${name}`)
  }
  const open = source.indexOf('{', start)
  let depth = 0
  for (let index = open; index < source.length; index += 1) {
    const char = source[index]
    if (char === '{') {
      depth += 1
    } else if (char === '}') {
      depth -= 1
      if (depth === 0) {
        return source.slice(start, index + 1)
      }
    }
  }
  throw new Error(`unclosed ${name}`)
}

const digitAt = (source: string, value: number, exp: number): number => {
  const tableFn = extractFunction(source, 'pow10_table')
  const digitFn = extractFunction(source, 'digit_at')
  if (/\bpow\s*\(/.test(tableFn) || /\bpow\s*\(/.test(digitFn)) {
    throw new Error('digit path calls pow')
  }
  if (!digitFn.includes('pow10_table')) {
    throw new Error('digit_at ignores pow10_table')
  }
  if (!/if\s*\(\s*exp\s*>=\s*0\s*\)\s*\{\s*scaled\s*=\s*value\s*\/\s*pow10_table\(\s*exp\s*\)\s*;\s*\}\s*else\s*\{\s*scaled\s*=\s*value\s*\*\s*pow10_table\(\s*-exp\s*\)\s*;\s*\}[\s\S]*return\s+u32\(\s*floor\(\s*scaled\s*\)\s*\)\s*%\s*10u\s*;/.test(digitFn)) {
    throw new Error('digit_at no longer scales through pow10_table')
  }
  const table = new Map<number, number>()
  for (const match of tableFn.matchAll(/case\s+(-?\d+)\s*:\s*\{\s*return\s+([0-9]*\.?[0-9]+)\s*;\s*\}/g)) {
    table.set(Number(match[1]), f32(Number(match[2])))
  }
  for (const exponent of [5, 4, 3, 2, 1, 0, -1, -2, -3, -4, -5]) {
    if (!table.has(exponent)) {
      throw new Error(`missing power ${exponent}`)
    }
  }
  const power = (exponent: number): number => table.get(exponent) ?? f32(1)
  const magnitude = f32(value)
  const scaled = exp >= 0 ? f32(magnitude / power(exp)) : f32(magnitude * power(-exp))
  return Math.floor(scaled) % 10
}

const readShader = (name: string): Promise<string> =>
  fs.readFile(path.join(rootDir, 'src', 'gpu', 'shaders', name), 'utf8')

test('popup value digit_at(1, -5) is zero', async () => {
  const shader = await readShader('popup_value.wgsl')

  assert.equal(digitAt(shader, 1, -5), 0)
})

test('probability popup digit_at(1, -5) is zero', async () => {
  const shader = await readShader('probability_popup_value.wgsl')

  assert.equal(digitAt(shader, 1, -5), 0)
})

export {}
