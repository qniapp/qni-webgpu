/* ─────────────────────────────────────────────────────────────────────
 * write0-gate.js — <write0-gate> カスタム要素
 *
 * |0⟩ オペレーションの正規描画を 1 箇所へ集約する。透明な 40px 領域に
 * tx-2 のケット括弧と red-600 の数字 0 を描き、state 属性で状態を表す。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const TEMPLATE = `
<style>
  :host {
    display: inline-flex;
    width: 40px;                          /* GATE_SIZE = spacing-10 */
    height: 40px;
    position: relative;
    align-items: center;
    justify-content: center;
    background: transparent;
    user-select: none;
    cursor: grab;
  }
  :host([state="drag"]) { cursor: grabbing; }

  .hover-frame {
    position: absolute;
    inset: -4px;                          /* body_rect.expand(4.0) */
    border: 2px solid var(--purple-400);  /* Flexoki purple-400 / gate_hover_border */
    border-radius: 10px;
    pointer-events: none;
    display: none;
  }
  :host([state="hover"]) .hover-frame,
  :host(:not([state]):hover) .hover-frame { display: block; }

  svg {
    display: block;
    width: 40px;
    height: 40px;
    pointer-events: none;
    overflow: visible;
  }
</style>
<div class="hover-frame"></div>
<svg viewBox="0 0 48 48" aria-hidden="true">
  <line x1="6" y1="5" x2="6" y2="43" stroke="var(--tx-2)" stroke-width="2"/>
  <line x1="37.4516" y1="5" x2="43.5" y2="24" stroke="var(--tx-2)" stroke-width="2"/>
  <line x1="43.5" y1="24" x2="37.4516" y2="43" stroke="var(--tx-2)" stroke-width="2"/>
  <g transform="translate(15.648 33.883) scale(0.027840 -0.027840)" fill="var(--red-600)">
    <path d="M300 -16Q223 -16 166.5 29.0Q110 74 80.0 157.0Q50 240 50 354Q50 469 80.0 552.0Q110 635 166.5 680.5Q223 726 300 726Q378 726 434.0 680.5Q490 635 520.0 552.0Q550 469 550 354Q550 240 520.0 157.0Q490 74 434.0 29.0Q378 -16 300 -16ZM140 354Q140 254 165 184L393 599Q354 642 300 642Q226 642 183.0 564.5Q140 487 140 354ZM300 68Q374 68 417.0 145.5Q460 223 460 354Q460 455 435 525L207 111Q246 68 300 68Z"/>
  </g>
</svg>
`;

  class Write0Gate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('write0-gate', Write0Gate);
})();
