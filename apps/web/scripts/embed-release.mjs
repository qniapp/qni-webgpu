import { createHash } from 'node:crypto'
import { execFileSync } from 'node:child_process'
import { copyFile, lstat, mkdir, readFile, readdir, writeFile } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parseArgs } from 'node:util'

const root = fileURLToPath(new URL('../../../', import.meta.url))

export class EmbedRelease {
  static assets = [
    ['qni-web.js', null, 'text/javascript'],
    ['qni-web_bg.wasm', null, 'application/wasm'],
    ['qni-embed.mjs', null, 'text/javascript'],
    ['qni-embed-error.mjs', null, 'text/javascript'],
    ['h.svg', 'apps/web/assets/icons/h.svg', 'image/svg+xml'],
    ['design-system.css', 'docs/design-system/design-system.css', 'text/css'],
    ['h-gate.js', 'docs/design-system/components/h-gate.js', 'text/javascript'],
    ['LICENSE', 'LICENSE', 'text/plain'],
  ]

  static hash(bytes) {
    return createHash('sha256').update(bytes).digest('hex')
  }

  static checkCommit(commit) {
    if (!/^[0-9a-f]{40}$/.test(commit ?? '')) throw new Error('source_commit must be 40 lowercase hex characters')
  }

  static async identity(directory, sourceCommit) {
    const source = await readFile(path.join(directory, 'qni-embed.mjs'), 'utf8')
    const version = source.match(/^export const EMBED_API_VERSION = ([1-9][0-9]*)\r?$/m)
    const commit = source.match(/^const buildSourceCommit = '([0-9a-f]{40})'\r?$/m)
    const sourceExport = "export const SOURCE_COMMIT = /^[0-9a-f]{40}$/i.test(buildSourceCommit) ? buildSourceCommit : 'unknown'"
    if (!source.split(/\r?\n/).includes(sourceExport) || commit?.[1] !== sourceCommit) throw new Error('built SOURCE_COMMIT mismatch or unsupported export expression')
    const apiVersion = Number(version?.[1])
    if (!Number.isSafeInteger(apiVersion) || apiVersion < 1) throw new Error('invalid EMBED_API_VERSION')
    return apiVersion
  }

  static async pinnedWasmTools() {
    const lock = await readFile(path.join(root, 'apps/web/Cargo.lock'), 'utf8')
    const bindgen = lock.match(/\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/)
    const config = await readFile(path.join(root, 'apps/web/Trunk.toml'), 'utf8')
    const opt = config.match(/^wasm_opt = "([^"]+)"/m)
    if (!bindgen || !opt) throw new Error('missing pinned wasm toolchain versions')
    return { 'wasm-bindgen': bindgen[1], 'wasm-opt': opt[1] }
  }

  static async toolchain() {
    const command = name => execFileSync(name, ['--version'], { encoding: 'utf8' }).trim()
    return { rustc: command('rustc'), ...await this.pinnedWasmTools(), trunk: command('trunk'), node: command(process.execPath), pnpm: command('pnpm') }
  }

  static async stage({ input, output, sourceCommit, toolchain }) {
    this.checkCommit(sourceCommit)
    const embedApiVersion = await this.identity(input, sourceCommit)
    await mkdir(output, { recursive: true })
    if ((await readdir(output)).length) throw new Error('output directory must be empty')
    const files = []
    for (const [name, source, contentType] of this.assets) {
      const destination = path.join(output, name)
      await copyFile(source ? path.join(root, source) : path.join(input, name), destination)
      const bytes = await readFile(destination)
      files.push({ name, sha256: this.hash(bytes), size: bytes.length, content_type: contentType })
    }
    const manifest = {
      schema_version: 1,
      source_commit: sourceCommit,
      embed_api_version: embedApiVersion,
      built_at: new Date().toISOString(),
      toolchain: toolchain ?? await this.toolchain(),
      files,
    }
    const bytes = `${JSON.stringify(manifest, null, 2)}\n`
    await writeFile(path.join(output, 'manifest.json'), bytes)
    await writeFile(path.join(output, 'manifest.json.sha256'), `${this.hash(bytes)}  manifest.json\n`)
    await this.verify(output, sourceCommit)
    return manifest
  }

  static async verify(output, expectedSourceCommit) {
    const bytes = await readFile(path.join(output, 'manifest.json'))
    const checksum = await readFile(path.join(output, 'manifest.json.sha256'), 'utf8')
    if (checksum !== `${this.hash(bytes)}  manifest.json\n`) throw new Error('manifest checksum mismatch')
    const manifest = JSON.parse(bytes)
    this.checkCommit(manifest.source_commit)
    if (expectedSourceCommit !== undefined) {
      this.checkCommit(expectedSourceCommit)
      if (manifest.source_commit !== expectedSourceCommit) throw new Error('source_commit mismatch')
    }
    if (manifest.schema_version !== 1 || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{3})?Z$/.test(manifest.built_at)) throw new Error('manifest schema mismatch')
    const tools = ['rustc', 'wasm-bindgen', 'wasm-opt', 'trunk', 'node', 'pnpm']
    if (!tools.every(name => typeof manifest.toolchain?.[name] === 'string' && manifest.toolchain[name].length > 0)) throw new Error('toolchain schema mismatch')
    const names = this.assets.map(([name]) => name)
    if (JSON.stringify(manifest.files?.map(file => file.name)) !== JSON.stringify(names)) throw new Error('manifest asset list mismatch')
    const actualNames = (await readdir(output)).sort()
    const expectedNames = [...names, 'manifest.json', 'manifest.json.sha256'].sort()
    if (JSON.stringify(actualNames) !== JSON.stringify(expectedNames)) throw new Error('directory asset list mismatch')
    for (const [index, file] of manifest.files.entries()) {
      const filename = path.join(output, file.name)
      if (!(await lstat(filename)).isFile()) throw new Error(`asset is not a regular file: ${file.name}`)
      const data = await readFile(filename)
      if (file.sha256 !== this.hash(data) || file.size !== data.length || file.content_type !== this.assets[index][2]) throw new Error(`asset checksum or metadata mismatch: ${file.name}`)
    }
    if (manifest.embed_api_version !== await this.identity(output, manifest.source_commit)) throw new Error('embed_api_version mismatch')
    return manifest
  }

  static async main(args) {
    const { values, positionals } = parseArgs({ args, allowPositionals: true, options: { input: { type: 'string' }, output: { type: 'string' }, 'source-commit': { type: 'string' } } })
    const [mode] = positionals
    if (positionals.length !== 1 || !['build', 'dry-run', 'verify'].includes(mode) || !values.output) throw new Error('usage: embed-release.mjs build|dry-run|verify --output DIR [--input DIR] [--source-commit SHA]')
    const output = path.resolve(values.output)
    if (mode === 'verify') return this.verify(output, values['source-commit'])
    this.checkCommit(values['source-commit'])
    const input = path.resolve(values.input ?? path.join(root, 'apps/web/embed-dist'))
    if (mode === 'build') execFileSync('bash', [path.join(root, 'apps/web/scripts/build-embed.sh'), input], { stdio: 'inherit', env: { ...process.env, QNI_SOURCE_COMMIT: values['source-commit'] } })
    return this.stage({ input, output, sourceCommit: values['source-commit'] })
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  EmbedRelease.main(process.argv.slice(2)).catch(error => {
    console.error(error.message)
    process.exitCode = 1
  })
}
