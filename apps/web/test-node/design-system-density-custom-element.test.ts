const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs/promises')
const path = require('node:path')

const repoRoot = path.join(__dirname, '..', '..', '..')

async function read(relativePath: string) {
  return fs.readFile(path.join(repoRoot, relativePath), 'utf8')
}

test('密度行列表示ブロックのカスタム要素を登録する', async () => {
  const source = await read('docs/design-system/components/density-matrix-display.js')

  assert.match(source, /customElements\.define\(ELEMENT_NAME/)
})

test('密度行列表示ブロック仕様はカスタム要素スクリプトを読み込む', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.match(source, /<script src="components\/density-matrix-display\.js"><\/script>/)
})

test('密度行列表示ブロックのカスタム要素はベースライン余白を出さない', async () => {
  const source = await read('docs/design-system/components/density-matrix-display.js')

  assert.match(source, /:host \{[\s\S]*line-height: 0;/)
})

test('密度行列表示ブロック仕様は旧 dm-host を使わない', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.doesNotMatch(source, /class="[^"]*dm-host/)
})

test('ゲートパレットは密度行列表示ブロックのカスタム要素を読み込む', async () => {
  const source = await read('docs/design-system/gate-palette.html')

  assert.match(source, /<script src="components\/density-matrix-display\.js"><\/script>/)
})

test('ゲートパレットは密度行列表示ブロックのカスタム要素を使う', async () => {
  const source = await read('docs/design-system/gate-palette.html')

  assert.match(source, /<density-matrix-display\b[^>]*\bicon\b/)
})

test('ゲートパレットは旧 g-Density シンボルを参照しない', async () => {
  const source = await read('docs/design-system/gate-palette.html')

  assert.doesNotMatch(source, /href="#g-Density"/)
})

test('垂直ディバイダ仕様は密度行列表示ブロックのカスタム要素を読み込む', async () => {
  const source = await read('docs/design-system/vertical-divider.html')

  assert.match(source, /<script src="components\/density-matrix-display\.js"><\/script>/)
})

test('垂直ディバイダ仕様は見た目セクションの密度行列アイコンをカスタム要素で描く', async () => {
  const source = await read('docs/design-system/vertical-divider.html')

  assert.match(source, /<density-matrix-display\b[^>]*\bicon\b/)
})

test('垂直ディバイダ仕様は旧密度行列アイコン用クラスを使わない', async () => {
  const source = await read('docs/design-system/vertical-divider.html')

  assert.doesNotMatch(source, /vd-cell--matrix/)
})

export {}
