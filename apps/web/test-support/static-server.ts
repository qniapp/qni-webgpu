import { createServer } from 'node:http'
import { readFile, realpath, stat } from 'node:fs/promises'
import { extname, isAbsolute, relative, resolve, sep } from 'node:path'

const contentTypes: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.css': 'text/css; charset=utf-8',
  '.ttf': 'font/ttf',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.json': 'application/json; charset=utf-8',
}

function isOutside(root: string, file: string): boolean {
  const path = relative(root, file)
  return path === '..' || path.startsWith(`..${sep}`) || isAbsolute(path)
}

export function createStaticServer(rootDir: string) {
  const root = resolve(rootDir)
  return createServer(async (req, res) => {
    let pathname: string
    try {
      pathname = decodeURIComponent((req.url ?? '/').split('?')[0])
      if (pathname.includes('\0')) throw new Error('Invalid path')
    } catch {
      res.writeHead(400).end()
      return
    }
    let file = resolve(root, `.${pathname}`)
    if (isOutside(root, file)) {
      res.writeHead(403).end()
      return
    }
    try {
      try {
        if (!(await stat(file)).isFile()) file = resolve(root, 'index.html')
      } catch (error) {
        if (!['ENOENT', 'ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? '')) throw error
        file = resolve(root, 'index.html')
      }
      const [realRoot, realFile] = await Promise.all([realpath(root), realpath(file)])
      if (isOutside(realRoot, realFile)) {
        res.writeHead(403).end()
        return
      }
      const body = await readFile(realFile)
      res.writeHead(200, { 'Content-Type': contentTypes[extname(file)] ?? 'application/octet-stream' })
      res.end(body)
    } catch {
      res.writeHead(500).end()
    }
  })
}
