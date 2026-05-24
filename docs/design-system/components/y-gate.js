/* ─────────────────────────────────────────────────────────────────────
 * y-gate.js — <y-gate> カスタム要素
 *
 * 仕様: docs/design-system/y-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから Y ゲートを参照するときは、簡略マークアップ
 * (色付き四角 + テキストラベル) や inline SVG ではなく、本要素を使って canonical な
 * 見た目をそのまま埋め込む。これにより y-gate.html の仕様変更が他ページへ自動追従する。
 *
 * 使い方:
 *   <script src="components/y-gate.js"></script>
 *
 *   <y-gate></y-gate>                <!-- 既定 (rest) -->
 *   <y-gate state="hover"></y-gate>  <!-- ホバー枠 + 背景の空きを強制 -->
 *   <y-gate state="drag"></y-gate>   <!-- drag preview (本体 purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が `.y-gate` などを再定義しても見た目が壊れない
 * - SVG path は実装 (apps/web/assets/icons/y.svg) と一致する Geist 700 outline
 * - Flexoki の CSS 変数 (--cyan-400 / --bg / --purple-400 / --purple-600) は
 *   :root から Shadow DOM へ inheritance で伝播するため、Shadow root 内で var() で参照できる
 * - ホバー枠は ::before / ::after の代わりに <div class="hover-frame"> / <div class="hover-gap">
 *   を使い、source order と body の不透明背景で「枠 + 空き + 本体」の 3 層を構成する
 *   (z-index を使わずに済むため、ホスト側の stacking context 干渉を受けない)
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const TEMPLATE = `
<style>
  :host {
    display: inline-block;
    width: 40px;                          /* GATE_SIZE = spacing-10 */
    height: 40px;
    position: relative;
    user-select: none;
    cursor: grab;
  }
  :host([state="drag"]) { cursor: grabbing; }

  /* ホバー枠 (purple-400) + 背景の空き (paper)。
     source order が body より先なので、body (本体) の背景で内側を隠し、
     枠と空きの「ring」だけが host の外側に visible になる。 */
  .hover-frame,
  .hover-gap {
    position: absolute;
    display: none;
    pointer-events: none;
  }
  .hover-frame {
    inset: -4px;                          /* body_rect.expand(4.0) */
    border-radius: 10px;
    background: var(--purple-400);        /* Flexoki purple-400 / gate_hover_border */
  }
  .hover-gap {
    inset: -2px;                          /* 2 px の paper 帯 */
    border-radius: 8px;
    background: var(--bg);                /* Flexoki bg / paper */
  }
  :host([state="hover"]) .hover-frame,
  :host(:not([state]):hover) .hover-frame { display: block; }
  :host([state="hover"]) .hover-gap,
  :host(:not([state]):hover) .hover-gap { display: block; }

  /* 本体 (cyan-400) + paper グリフ。y-gate.html § 02 / § 03 と完全一致。 */
  .body {
    position: absolute;
    inset: 0;
    background: var(--cyan-400);          /* Flexoki cyan-400 / box_fill */
    border-radius: 6px;                   /* rounded-md = CornerRadius::same(6) */
    color: var(--bg);                     /* paper グリフ (SVG path の currentColor) */
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 120ms ease-out;
  }
  :host([state="drag"]) .body {
    background: var(--purple-600);        /* Flexoki purple-600 / drag_fill */
  }
  .body > svg {
    width: 100%;
    height: 100%;
    display: block;
    pointer-events: none;
  }
</style>
<div class="hover-frame"></div>
<div class="hover-gap"></div>
<div class="body">
  <!-- Geist 700 の "Y" outline。path d は apps/web/assets/icons/y.svg と完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <g transform="translate(15.429 34.565) scale(0.029760 -0.029760)">
      <path d="M245 0V290L-6 710H96L288 378L480 710H582L331 290V0Z" fill="currentColor"/>
    </g>
  </svg>
</div>
`;

  class YGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('y-gate', YGate);
})();
