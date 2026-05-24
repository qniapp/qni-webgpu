/* ─────────────────────────────────────────────────────────────────────
 * control-gate.js — <control-gate> カスタム要素
 *
 * 制御マーカの正規描画を 1 箇所へ集約する。透明な 40px 領域の中央に
 * cyan-400 の塗りつぶし円を置き、state 属性でホバー / ドラッグ状態を表す。
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
    color: var(--cyan-400);               /* Flexoki cyan-400 / box_fill */
    background: transparent;
    user-select: none;
    cursor: grab;
  }
  :host([state="drag"]) {
    color: var(--purple-600);             /* Flexoki purple-600 / drag_fill */
    cursor: grabbing;
  }

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

  .body {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .body > svg {
    width: 100%;
    height: 100%;
    display: block;
    pointer-events: none;
    overflow: visible;
  }
</style>
<div class="hover-frame"></div>
<div class="body">
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <circle cx="24" cy="24" r="8" fill="currentColor"/>
  </svg>
</div>
`;

  class ControlGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('control-gate', ControlGate);
})();
