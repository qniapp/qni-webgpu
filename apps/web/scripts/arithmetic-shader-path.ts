import { execFileSync } from 'node:child_process'
import { readFileSync, realpathSync, statSync } from 'node:fs'
import { join } from 'node:path'

let cached: string | undefined
export function arithmeticShaderPath(): string {
  if (cached) return cached
  const cwd = join(__dirname, '..')
  const cargo = (args: string[]) => {
    try {
      return execFileSync('cargo', args, {
        cwd, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, stdio: ['ignore', 'pipe', 'inherit'],
      })
    } catch (error) {
      // Cargo JSON sends compiler diagnostics to stdout, not inherited stderr.
      for (const line of String((error as { stdout?: string }).stdout ?? '').split('\n')) {
        if (!line.startsWith('{')) continue
        const event = JSON.parse(line)
        if (event.reason === 'compiler-message' && event.message.rendered) process.stderr.write(event.message.rendered)
      }
      throw error
    }
  }
  const manifest = realpathSync(join(cwd, 'Cargo.toml'))
  const metadata = JSON.parse(cargo(['metadata', '--locked', '--no-deps', '--format-version=1']))
  const packages = metadata.packages.filter((p: { name: string; manifest_path: string }) =>
    p.name === 'qni-web' && realpathSync(p.manifest_path) === manifest)
  if (packages.length !== 1) throw Error('Expected exactly one local qni-web package')
  const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8' }).match(/^host: (.+)$/m)?.[1]
  if (!host) throw Error('Cannot determine Rust host target')
  // Explicit host target overrides any wasm default. Cargo performs freshness checks even on cache hits.
  const events = cargo(['build', '--locked', '--tests', '--package', packages[0].id,
    '--target', host, '--message-format=json']).trim().split('\n').filter(Boolean).map((line) => JSON.parse(line))
  const outputs = events.filter((e) => e.reason === 'build-script-executed' && e.package_id === packages[0].id)
  if (outputs.length !== 1) throw Error(`Expected one qni-web build-script output, got ${outputs.length}`)
  const path = join(outputs[0].out_dir, 'arithmetic.wgsl')
  if (!statSync(path).isFile() || !readFileSync(path, 'utf8').trim()) throw Error(`Missing generated shader: ${path}`)
  cached = path
  return path
}
if (require.main === module) {
  try {
    if (process.argv.length !== 2) throw Error('Usage: arithmetic-shader-path.ts')
    console.log(arithmeticShaderPath())
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
