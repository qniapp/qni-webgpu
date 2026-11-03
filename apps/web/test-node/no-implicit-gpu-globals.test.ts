import test from 'node:test'
import assert from 'node:assert/strict'
import path from 'node:path'
import ts from 'typescript'
import { implicitGpuGlobals } from '../scripts/check-no-implicit-gpu-globals'

test('Node targets use explicit WebGPU constants, with scope-aware sample coverage', () => {
  const samples: [string, string, number][] = [
    ['bare', 'GPUBufferUsage.STORAGE', 1],
    ['map', 'GPUMapMode.READ', 1],
    ['stage', 'GPUShaderStage.COMPUTE', 1],
    ['texture', 'GPUTextureUsage.COPY_DST', 1],
    ['color', 'GPUColorWrite.ALL', 1],
    ['global', 'globalThis.GPUBufferUsage.STORAGE', 1],
    ['global-index', 'globalThis["GPUMapMode"].READ', 1],
    ['global-destructure', 'const { GPUBufferUsage } = globalThis; GPUBufferUsage.STORAGE', 1],
    ['global-destructure-alias', 'const { GPUMapMode: M } = (globalThis as any)', 1],
    ['global-parameter', 'function f({ GPUMapMode } = globalThis) {}', 1],
    ['satisfies', '(globalThis satisfies any).GPUMapMode.READ', 1],
    ['instantiation', 'GPUMapMode<any>', 1],
    ['ambient-local', 'declare const GPUMapMode: any; GPUMapMode.READ', 1],
    ['ambient-merge', 'declare global { interface GPUMapMode {} }; GPUMapMode.READ', 1],
    ['ambient-global', 'declare global { var GPUColorWrite: any }; GPUColorWrite.ALL', 1],
    ['global-assignment', 'let M: any; ({ GPUMapMode: M } = globalThis)', 1],
    ['global-computed-binding', 'const { ["GPUMapMode"]: M } = globalThis', 1],
    ['shorthand', 'const x = { GPUBufferUsage }', 1],
    ['computed', 'const x = { [GPUMapMode.READ]: 1 }', 1],
    ['scope', '{ const GPUMapMode = {READ: 1} }; GPUMapMode.READ', 1],
    ['type-import-value', 'import type { GPUMapMode } from "webgpu"; GPUMapMode.READ', 1],
    ['type-alias-value', 'type GPUMapMode = number; GPUMapMode.READ', 1],
    ['local', 'const GPUMapMode = {READ: 1}; GPUMapMode.READ', 0],
    ['parameter', 'function f(GPUMapMode: any) { return GPUMapMode.READ }', 0],
    ['destructured', 'const { GPUBufferUsage } = globals; GPUBufferUsage.STORAGE', 0],
    ['imported', 'import { GPUMapMode } from "webgpu"; GPUMapMode.READ', 0],
    ['default-import', 'import GPUMapMode from "webgpu"; GPUMapMode.READ', 0],
    ['namespace-import', 'import * as GPUMapMode from "webgpu"; GPUMapMode.READ', 0],
    ['require-import', 'import GPUMapMode = require("webgpu"); GPUMapMode.READ', 0],
    ['type-specifier', 'import { type GPUMapMode } from "webgpu"; GPUMapMode.READ', 1],
    ['heritage-types', 'interface X extends GPUMapMode {} class Y implements GPUMapMode {}', 0],
    ['import-alias', 'import { globals as GPUMapMode } from "webgpu"; GPUMapMode.READ', 0],
    ['types', 'type X = typeof GPUMapMode; let x: GPUBufferUsage; interface Y { GPUColorWrite: number }', 0],
    ['text', '// GPUMapMode.READ\nconst x = "GPUBufferUsage.STORAGE"', 0],
    ['properties', 'const x = { GPUMapMode: 1 }; x.GPUMapMode', 0],
    ['explicit', '(globals as typeof globalThis).GPUBufferUsage.STORAGE', 0],
    ['shadow-global', 'function f(globalThis: any) { return globalThis.GPUMapMode.READ }', 0],
  ]
  const sources = new Map(samples.map(([name, source]) => [path.resolve(`${name}.ts`), `${source}\nexport {}`]))
  sources.set(path.resolve('browser.d.ts'), 'declare const GPUMapMode: { READ: number }')
  const options = { noLib: true, noResolve: true }
  const host = ts.createCompilerHost(options)
  host.getSourceFile = (file, languageVersion) => {
    const source = sources.get(file)
    return source === undefined ? undefined : ts.createSourceFile(file, source, languageVersion, true)
  }
  const fixtures = ts.createProgram([...sources.keys()], options, host)
  const configPath = path.join(__dirname, '..', 'tsconfig.json')
  const config = ts.readConfigFile(configPath, ts.sys.readFile)
  if (config.error) throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, path.dirname(configPath))
  if (parsed.errors.length) throw new Error(ts.formatDiagnosticsWithColorAndContext(parsed.errors, host))
  // Scan configured Node entry modules, not the browser bootstrap or declarations.
  const browserBootstrap = path.join(path.dirname(configPath), 'bootstrap.ts')
  const targets = parsed.fileNames.filter(file => path.resolve(file) !== browserBootstrap && !file.endsWith('.d.ts'))
  const repo = ts.createProgram(targets, parsed.options)
  assert.deepEqual({
    samples: samples.map(([name]) => [name, implicitGpuGlobals(fixtures, fixtures.getSourceFile(path.resolve(`${name}.ts`))!).length]),
    repo: targets.flatMap(file => implicitGpuGlobals(repo, repo.getSourceFile(file)!)),
  }, { samples: samples.map(([name, , count]) => [name, count]), repo: [] })
})
