/* ─────────────────────────────────────────────────────────────────────
 * measurement-gate.js — <measurement-gate> カスタム要素
 *
 * 測定ゲートの正規描画を 1 箇所へ集約する。透明な 40px 領域に purple-600 の
 * メーターを描き、outcome="0" / "1" で測定済みの数字表示へ切り替える。
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
    color: var(--purple-600);             /* Flexoki purple-600 / semantic_intermediate */
    user-select: none;
    cursor: grab;
  }
  :host([state="drag"]) { cursor: grabbing; }
  :host([outcome="0"]),
  :host([outcome="1"]) { color: var(--ui-2); } /* measurement_fired_icon */

  .hover-frame {
    position: absolute;
    inset: -4px;
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
  .digit-layer {
    position: absolute;
    inset: 0;
    z-index: 1;
    transform: translateY(1px);           /* MEASUREMENT_DIGIT_CENTER_Y_OFFSET */
    display: none;
  }
  :host([outcome="0"]) .digit-zero,
  :host([outcome="1"]) .digit-one { display: block; }
  .digit-zero { color: var(--red-600); fill: var(--red-600); }
  .digit-one { color: var(--blue-600); fill: var(--blue-600); }
</style>
<div class="hover-frame"></div>
<svg class="meter" viewBox="0 0 48 48" aria-hidden="true">
  <path d="M 4 36 A 20 20 0 0 1 44 36" fill="none" stroke="currentColor" stroke-width="2"/>
  <line x1="24.625" y1="33.5" x2="37.75" y2="11" stroke="currentColor" stroke-width="2"/>
  <circle cx="24.625" cy="33.5" r="3.5" fill="currentColor"/>
</svg>
<svg class="digit-layer digit-zero" viewBox="0 0 48 48" aria-hidden="true">
  <g transform="translate(15.648 33.883) scale(0.027840 -0.027840)">
    <path d="M300 -16Q223 -16 166.5 29.0Q110 74 80.0 157.0Q50 240 50 354Q50 469 80.0 552.0Q110 635 166.5 680.5Q223 726 300 726Q378 726 434.0 680.5Q490 635 520.0 552.0Q550 469 550 354Q550 240 520.0 157.0Q490 74 434.0 29.0Q378 -16 300 -16ZM140 354Q140 254 165 184L393 599Q354 642 300 642Q226 642 183.0 564.5Q140 487 140 354ZM300 68Q374 68 417.0 145.5Q460 223 460 354Q460 455 435 525L207 111Q246 68 300 68Z"/>
  </g>
</svg>
<svg class="digit-layer digit-one" viewBox="0 0 48 48" aria-hidden="true">
  <g transform="translate(15.648 33.883) scale(0.027840 -0.027840)">
    <path d="M60 0V84H284V526H98V600H194Q251 600 275.5 625.0Q300 650 300 710H370V84H540V0Z"/>
  </g>
</svg>
`;

  class MeasurementGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('measurement-gate', MeasurementGate);
})();
