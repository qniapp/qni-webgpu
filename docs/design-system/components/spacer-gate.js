/* ─────────────────────────────────────────────────────────────────────
 * spacer-gate.js — <spacer-gate> カスタム要素
 *
 * Spacer ゲートの正規描画を 1 箇所へ集約する。透明な 40px 領域に tx の
 * 3 点だけを描き、state="hover" / frame="palette" で枠の種類を表す。
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
    color: var(--tx);                     /* Flexoki tx / spacer_dots */
    user-select: none;
    cursor: grab;
    isolation: isolate;
  }
  :host([state="drag"]) { cursor: grabbing; }

  .circuit-frame {
    position: absolute;
    inset: -4px;
    border: 2px solid var(--purple-400);  /* Flexoki purple-400 / gate_hover_border */
    border-radius: 10px;
    pointer-events: none;
    display: none;
  }
  :host([state="hover"]) .circuit-frame,
  :host(:not([state]):not([frame="palette"]):hover) .circuit-frame { display: block; }

  .palette-outer,
  .palette-inner {
    position: absolute;
    pointer-events: none;
    display: none;
  }
  .palette-outer {
    inset: -4px;
    border-radius: 10px;
    background: var(--purple-400);
    z-index: -2;
  }
  .palette-inner {
    inset: -2px;
    border-radius: 8px;
    background: var(--bg);
    z-index: -1;
  }
  :host([frame="palette"]) .palette-outer,
  :host([frame="palette"]) .palette-inner { display: block; }

  svg {
    display: block;
    width: 40px;
    height: 40px;
    pointer-events: none;
    overflow: visible;
  }
</style>
<div class="circuit-frame"></div>
<div class="palette-outer"></div>
<div class="palette-inner"></div>
<svg viewBox="0 0 48 48" aria-hidden="true">
  <rect x="9" y="21" width="6" height="6" fill="currentColor"/>
  <rect x="21" y="21" width="6" height="6" fill="currentColor"/>
  <rect x="33" y="21" width="6" height="6" fill="currentColor"/>
</svg>
`;

  class SpacerGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('spacer-gate', SpacerGate);
})();
