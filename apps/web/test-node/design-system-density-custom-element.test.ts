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

test('密度行列表示ブロック仕様は振幅表示ブロックとの差分表を持つ', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.match(source, /data-density-comparison="amplitude-gap"/)
})

test('密度行列表示ブロック仕様は span 2 の中間混合状態見本を持つ', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.match(source, /span="2" state="mixed50"/)
})

test('密度行列表示ブロックのカスタム要素は中間混合状態を半分の Bell 混合として描く', async () => {
  const source = await read('docs/design-system/components/density-matrix-display.js')

  assert.match(source, /name === 'mixed50'[\s\S]*#fillBellMix\(buf, n, 0\.5\)/)
})

test('密度行列表示ブロックのカスタム要素は吹き出し型ポップオーバー外装を使う', async () => {
  const source = await read('docs/design-system/components/density-matrix-display.js')

  assert.match(source, /dm-popover[\s\S]*doc-popover-tail/)
})

test('密度行列表示ブロック仕様はポップオーバー仕様ページへリンクする', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.match(source, /href="density-matrix-display-popover\.html"/)
})

test('密度行列表示ブロック仕様は旧ポップアップ実例を持たない', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.doesNotMatch(source, /class="dm-popup"/)
})

test('密度行列表示ブロックのカスタム要素は旧ポップアップ API 名を残さない', async () => {
  const source = await read('docs/design-system/components/density-matrix-display.js')

  assert.doesNotMatch(source, /dm-popup|#attachPopup|#popupHtml/)
})

test('密度行列表示ポップオーバー仕様は共通 tail を持つ', async () => {
  const source = await read('docs/design-system/density-matrix-display-popover.html')

  assert.match(source, /doc-popover-tail doc-popover-tail--left/)
})

test('デザインシステムは密度行列表示ポップオーバーをコンポーネント別ポップオーバーに載せる', async () => {
  const source = await read('docs/design-system.html')

  assert.match(source, /href="design-system\/density-matrix-display-popover\.html"/)
})

test('密度行列表示ブロック仕様は実装仕様ページへリンクする', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.match(source, /href="\.\.\/implementation\/density-matrix-display\.html"/)
})

test('密度行列表示ブロック仕様は Qiskit GPU 実装セクションを持たない', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.doesNotMatch(source, /<h2>§ 10 Qiskit \/ GPU 実装メモ/)
})

test('密度行列表示ブロック仕様は GPU バッファ名を本文に持たない', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.doesNotMatch(source, /density_output|density_meta|vec2&lt;f32&gt;|GPU storage|512 KB/)
})

test('密度行列表示ブロック仕様は配置衝突の実装語彙を本文に持たない', async () => {
  const source = await read('docs/design-system/density-matrix-display.html')

  assert.doesNotMatch(source, /gate_rect|\{"cols":\[\["Density3"\]\]\}/)
})

test('実装仕様インデックスは密度行列表示ブロック実装仕様へリンクする', async () => {
  const source = await read('docs/implementation.html')

  assert.match(source, /href="implementation\/density-matrix-display\.html"/)
})

test('実装仕様インデックスは Web 開発手順の実ファイルへリンクする', async () => {
  const source = await read('docs/implementation.html')

  assert.match(source, /href="web\.md"/)
})

test('密度行列表示ブロック実装仕様は Qiskit GPU 実装メモを持つ', async () => {
  const source = await read('docs/implementation/density-matrix-display.html')

  assert.match(source, /<h2>§ 01 Qiskit \/ GPU 実装メモ/)
})

test('密度行列表示ブロック実装仕様は配置衝突の実装詳細を持つ', async () => {
  const source = await read('docs/implementation/density-matrix-display.html')

  assert.match(source, /id="serialization-placement"[\s\S]*gate_rect/)
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
