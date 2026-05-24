/* ─────────────────────────────────────────────────────────────────────
 * s-gate.js — <s-gate> カスタム要素
 *
 * 仕様: docs/design-system/s-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから S を参照するときは、inline SVG ではなく、
 * 本要素を使って canonical な見た目をそのまま埋め込む。
 *
 * 使い方:
 *   <script src="components/s-gate.js"></script>
 *
 *   <s-gate></s-gate>                <!-- 既定 (rest) -->
 *   <s-gate state="hover"></s-gate>  <!-- ホバー枠 + 背景の空きを強制 -->
 *   <s-gate state="drag"></s-gate>   <!-- drag preview (本体 purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が `.s-gate` などを再定義しても見た目が壊れない
 * - SVG path は実装 (apps/web/assets/icons/s.svg) と一致する Geist 700 outline
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
  <!-- S の glyph。path d は apps/web/assets/icons/s.svg と完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
<g transform="translate(14.477 34.565) scale(0.029760 -0.029760)">
    <path d="M336 -16Q254 -16 193.0 15.0Q132 46 96.5 100.5Q61 155 55 227L145 233Q156 154 204.5 111.0Q253 68 338 68Q412 68 453.5 96.5Q495 125 495 180Q495 213 479.5 238.5Q464 264 420.5 285.5Q377 307 292 328Q210 349 161.5 374.0Q113 399 91.5 435.0Q70 471 70 524Q70 584 98.5 629.5Q127 675 180.0 700.5Q233 726 306 726Q384 726 440.0 696.5Q496 667 529.0 616.0Q562 565 570 500L480 494Q471 559 427.5 600.5Q384 642 304 642Q238 642 199.0 610.5Q160 579 160 528Q160 495 175.5 473.5Q191 452 231.0 435.5Q271 419 346 400Q435 378 487.5 347.5Q540 317 562.5 277.0Q585 237 585 186Q585 124 553.0 78.5Q521 33 465.0 8.5Q409 -16 336 -16Z" fill="currentColor"/>
  </g>
  </svg>
</div>
`;

  class SGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('s-gate', SGate);
})();
