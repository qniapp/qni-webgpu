import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { request } from 'node:http'
import { once } from 'node:events'
import { createStaticServer } from '../test-support/static-server'

async function fixture(t: test.TestContext) {
  const dir = await mkdtemp(join(tmpdir(), 'qni-static-'))
  t.after(() => rm(dir, { recursive: true, force: true }))
  const root = join(dir, 'root')
  await mkdir(root)
  await writeFile(join(root, 'index.html'), '<html>app</html>')
  await writeFile(join(root, 'a.wasm'), 'wasm')
  await writeFile(join(dir, 'secret.txt'), 'secret')
  const server = createStaticServer(root)
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  t.after(() => new Promise<void>((resolve, reject) => {
    server.close(error => error ? reject(error) : resolve())
  }))
  const address = server.address()
  if (!address || typeof address === 'string') throw new Error('Missing server port')
  return (path: string) => new Promise<{ status: number | undefined; type: string | undefined; body: string }>((resolve, reject) => {
    const req = request({ hostname: '127.0.0.1', port: address.port, path }, response => {
      let body = ''
      response.setEncoding('utf8')
      response.on('data', chunk => { body += chunk })
      response.on('end', () => resolve({ status: response.statusCode, type: response.headers['content-type'], body }))
      response.on('error', reject)
    })
    req.on('error', reject)
    req.end()
  })
}

test('serves wasm with the WebAssembly content type', async t => {
  const get = await fixture(t)
  assert.equal((await get('/a.wasm')).type, 'application/wasm')
})

test('ignores query strings when serving files', async t => {
  const get = await fixture(t)
  assert.equal((await get('/a.wasm?v=1')).body, 'wasm')
})

test('serves index.html for an encoded circuit path', async t => {
  const get = await fixture(t)
  const response = await get('/%7B%22cols%22:[]%7D')
  assert.deepEqual({ status: response.status, body: response.body }, { status: 200, body: '<html>app</html>' })
})

test('rejects traversal outside the root', async t => {
  const get = await fixture(t)
  assert.equal((await get('/%2e%2e/secret.txt')).status, 403)
})
