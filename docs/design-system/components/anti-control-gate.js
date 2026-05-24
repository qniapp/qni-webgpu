/* ─────────────────────────────────────────────────────────────────────
 * anti-control-gate.js — <anti-control-gate> カスタム要素
 *
 * 反制御マーカの正規描画を 1 箇所へ集約する。透明な 40px 領域の中央に
 * cyan-400 の空円を置き、--ac-mask で空円内側のワイヤ消し込みを制御する。
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

  .mask {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 8px;                           /* spacing-2 */
    height: 8px;
    border-radius: 999px;
    background: var(--ac-mask, transparent);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
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
    position: relative;
    z-index: 1;
  }
</style>
<div class="hover-frame"></div>
<div class="mask"></div>
<div class="body">
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <circle cx="24" cy="24" r="5" fill="none" stroke="currentColor" stroke-width="3"/>
  </svg>
</div>
`;

  class AntiControlGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('anti-control-gate', AntiControlGate);
})();
