// 軽量な DOM 代役で配置を検証する。実際の CSS 描画はブラウザで別途確認する。
const { test } = require('node:test');
const assert = require('node:assert/strict');
class Element {
  constructor(tag) { this.tagName = tag; this.children = []; this.style = {}; this.attrs = {}; }
  attachShadow() { return (this.shadowRoot = new Element('shadow')); }
  appendChild(child) { this.children.push(child); return child; }
  replaceChildren(...children) { this.children = children; }
  setAttribute(key, value) { this.attrs[key] = value; }
  getAttribute(key) { return this.attrs[key] ?? null; }
}
global.HTMLElement = Element;
global.document = { createElement: tag => new Element(tag) };
let Circuit;
global.customElements = { get: () => undefined, define: (_, ctor) => { Circuit = ctor; } };
require('../docs/design-system/components/quantum-circuit.js');
function render(cols) {
  const circuit = new Circuit();
  circuit.setAttribute('cols', JSON.stringify(cols));
  circuit.connectedCallback();
  return circuit.shadowRoot.children[1];
}
const children = (grid, tag) => grid.children.filter(child => child.tagName === tag);
test('マーカー列はセルを消費しない', () => {
  assert.equal(children(render([['{a'], ['H'], ['}'], [1], ['X']]), 'circuit-dropzone').length, 3);
});
test('ブロックは上下に同じラベルを置く', () => {
  assert.deepEqual(render([['{a'], ['H'], ['}']]).children.filter(c => c.className === 'block-label').map(c => c.textContent), ['a', 'a']);
});
test('ブロックは全ワイヤを覆う', () => {
  assert.equal(render([['{a'], ['H', 'X'], ['}']]).children.find(c => c.className === 'circuit-block')?.style.gridRow, '1 / span 2');
});
for (const [token, tag] of [['Probability', 'probability-display'], ['Density', 'density-matrix-display']]) {
  test(`${token} は span 1`, () => {
    assert.equal(children(render([[token]]), tag)[0]?.attrs.span, '1');
  });
}
for (const [span, width] of [[2, 2], [3, 2], [4, 4]]) {
  test(`Amps${span} は ${width} 列幅`, () => {
    assert.equal(children(render([[`Amps${span}`], [1], [1], [1]]), 'amplitude-display')[0].style.gridColumn, `2 / span ${width}`);
  });
}
for (const span of [1, 2, 3]) {
  test(`Sample${span} は ${span} ワイヤの箱`, () => {
    assert.equal(render([[`Sample${span}`]]).children.find(c => c.className === 'span-gate sample-placeholder')?.style.gridRow, `1 / span ${span}`);
  });
}
test('位相回転は P ゲートを描く', () => {
  const cell = children(render([['P(π_2)', '•']]), 'circuit-dropzone')[0];
  assert.equal(cell.children[0]?.children[0]?.tagName, 'p-gate');
});
test('位相回転は角度を表示する', () => {
  const cell = children(render([['P(π_2)', '•']]), 'circuit-dropzone')[0];
  assert.equal(cell.children[0]?.children[1]?.textContent, 'π/2');
});
test('閉じ忘れたブロックは回路末尾まで続く', () => {
  assert.equal(render([['{a'], ['H'], ['X']]).children.find(c => c.className === 'circuit-block').style.gridColumn, '2 / span 2');
});
for (const [name, cols] of [
  ['入れ子', [['{a'], ['{b'], ['H'], ['}'], ['}']]],
  ['空ラベル', [['{'], ['H'], ['}']]],
  ['開始のない終了', [['}']]],
]) {
  test(`不正なブロック（${name}）は表示しない`, () => {
    assert.equal(render(cols), undefined);
  });
}
test('空ブロックは帯を作らない', () => {
  assert.equal(render([['{a'], ['}'], ['H']]).children.filter(c => c.className === 'circuit-block').length, 0);
});
test('ブロックなしでは制御線の配置を維持する', () => {
  assert.deepEqual(children(render([['•', 'X']]), 'circuit-dropzone').map(c => c.attrs), [{ 'connect-bottom': '' }, { 'connect-top': '' }]);
});
test('ラベルは HTML としてではなく文字列として挿入する', () => {
  assert.equal(render([['{<b>"a"</b>'], ['H'], ['}']]).children.find(c => c.className === 'block-label').textContent, '<b>"a"</b>');
});
