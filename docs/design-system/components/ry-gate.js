/* ─────────────────────────────────────────────────────────────────────
 * ry-gate.js — <ry-gate> カスタム要素
 *
 * 仕様: docs/design-system/ry-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから RY を参照するときは、inline SVG ではなく、
 * 本要素を使って canonical な見た目をそのまま埋め込む。
 *
 * 使い方:
 *   <script src="components/ry-gate.js"></script>
 *
 *   <ry-gate></ry-gate>                <!-- 既定 (rest) -->
 *   <ry-gate state="hover"></ry-gate>  <!-- ホバー枠 + 背景の空きを強制 -->
 *   <ry-gate state="drag"></ry-gate>   <!-- drag preview (本体 purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が `.ry-gate` などを再定義しても見た目が壊れない
 * - SVG path は実装 (apps/web/assets/icons/ry.svg) と一致する Geist 700 outline
 * - Flexoki の CSS 変数 (--cyan-400 / --bg / --purple-400 / --purple-600) は
 *   :root から Shadow DOM へ inheritance で伝播する
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

  .body {
    position: absolute;
    inset: 0;
    background: var(--cyan-400);          /* Flexoki cyan-400 / box_fill */
    border-radius: 6px;         /* rounded-md = CornerRadius::same(6) */
    color: var(--bg);                     /* paper グリフ */
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
  <!-- RY の glyph。path d は apps/web/assets/icons/ry.svg と完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
<g transform="translate(8.908 31.838) scale(0.022080 -0.022080)">
    <path d="M86 0V710H369Q482 710 548.5 652.0Q615 594 615 496Q615 434 579.5 389.5Q544 345 494 330Q543 322 570.5 292.0Q598 262 603 207L622 0H513L496 193Q493 236 467.0 256.5Q441 277 382 277H194V0ZM194 376H371Q432 376 467.5 406.0Q503 436 503 493Q503 550 466.5 580.5Q430 611 362 611H194Z" fill="currentColor"/>
  </g>
  <g transform="translate(23.923 31.838) scale(0.022080 -0.022080)">
    <path d="M243 0V285L-6 710H118L297 393L477 710H601L351 285V0Z" fill="currentColor"/>
  </g>
  </svg>
</div>
`;

  class RyGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('ry-gate', RyGate);
})();
