const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const workflow = () => fs.readFile(path.join(__dirname, '..', '.github', 'workflows', 'pages.yml'), 'utf8')

const contracts = [
  ['runs on master pushes', /push:\n\s+branches: \[master\]/],
  ['allows manual runs', /\n  workflow_dispatch:/],
  ['grants contents read permission', /permissions:\n  contents: read/],
  ['grants Pages write permission', /\n  pages: write/],
  ['grants ID token write permission', /\n  id-token: write/],
  ['serializes deployments in the pages group', /concurrency:\n  group: pages/],
  ['does not cancel deployments in progress', /\n  cancel-in-progress: false/],
  ['installs the wasm target', /rustup target add wasm32-unknown-unknown/],
  ['installs Trunk', /tool: trunk/],
  ['installs frozen Web dependencies in the app directory', /pnpm install --frozen-lockfile\n\s+working-directory: apps\/web/],
  ['builds the standalone release at its repository subpath from the app directory', /trunk build --release --public-url \/qni-webgpu\/\n\s+working-directory: apps\/web/],
  ['configures Pages', /uses: actions\/configure-pages@v5/],
  ['uploads the standalone dist', /uses: actions\/upload-pages-artifact@v4\n\s+with:\n\s+path: apps\/web\/dist/],
  ['waits for build before deploying', /\n  deploy:\n\s+needs: build/],
  ['exposes the github-pages environment URL', /environment:\n\s+name: github-pages\n\s+url: \$\{\{ steps\.deployment\.outputs\.page_url \}\}/],
  ['deploys the Pages artifact', /id: deployment\n\s+uses: actions\/deploy-pages@v5/],
]

for (const [name, pattern] of contracts) {
  test(`Pages ${name}`, async () => {
    assert.match(await workflow(), pattern)
  })
}

test('Pages does not run on pull requests', async () => {
  assert.doesNotMatch(await workflow(), /pull_request/)
})
