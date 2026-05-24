/* ─────────────────────────────────────────────────────────────────────
 * write1-gate.js — <write1-gate> カスタム要素
 *
 * |1⟩ オペレーションの正規描画を 1 箇所へ集約する。透明な 40px 領域に
 * tx-2 のケット括弧と blue-600 の数字 1 を描き、state 属性で状態を表す。
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
  <g transform="translate(15.648 33.883) scale(0.027840 -0.027840)" fill="var(--blue-600)">
    <path d="M60 0V84H284V526H98V600H194Q251 600 275.5 625.0Q300 650 300 710H370V84H540V0Z"/>
  </g>
</svg>
`;

  class Write1Gate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('write1-gate', Write1Gate);
})();
