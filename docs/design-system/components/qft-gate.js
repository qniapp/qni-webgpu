/* ─────────────────────────────────────────────────────────────────────
 * qft-gate.js — <qft-gate> カスタム要素
 *
 * QFT ゲートの正規描画を 1 箇所へ集約する。span 属性または --span で高さを
 * 指定し、state 属性でホバー / ドラッグ状態、handles 属性でリサイズハンドルを表す。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const TEMPLATE = `
<style>
  :host {
    --span: 1;
    display: inline-flex;
    width: 40px;                                  /* GATE_SIZE = spacing-10 */
    height: calc(56px * var(--span) - 16px);      /* 56·span − 16 */
    position: relative;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    user-select: none;
    cursor: grab;
  }
  :host([state="drag"]) { cursor: grabbing; }

  .hover-outer,
  .hover-inner {
    position: absolute;
    pointer-events: none;
    display: none;
  }
  .hover-outer {
    inset: -4px;
    border-radius: 10px;
    background: var(--purple-400);                /* Flexoki purple-400 / gate_hover_border */
  }
  .hover-inner {
    inset: -2px;
    border-radius: 8px;
    background: var(--bg);                        /* Flexoki bg / paper */
  }
  :host([state="hover"]) .hover-outer,
  :host([state="hover"]) .hover-inner,
  :host(:not([state]):hover) .hover-outer,
  :host(:not([state]):hover) .hover-inner {
    display: block;
  }

  .body {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--cyan-400);                  /* Flexoki cyan-400 / box_fill */
    color: var(--bg);                             /* Flexoki bg / glyph */
    border-radius: 6px;
  }
  :host([state="drag"]) .body {
    background: var(--purple-600);                /* Flexoki purple-600 / drag_fill */
  }
  .body > svg {
    width: 40px;
    height: 40px;
    display: block;
    pointer-events: none;
    flex-shrink: 0;
  }

  .resize-handle {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    width: 24px;
    height: 6px;
    border-radius: 9999px;
    background: var(--purple-400);                /* Flexoki purple-400 / resize affordance */
    z-index: 2;
    display: none;
    pointer-events: none;
  }
  .resize-handle.top { top: -10px; }
  .resize-handle.bottom { bottom: -10px; }
  :host([handles][state="hover"]) .resize-handle,
  :host([handles][state="drag"]) .resize-handle,
  :host([handles]:not([state]):hover) .resize-handle {
    display: block;
  }
</style>
<div class="hover-outer"></div>
<div class="hover-inner"></div>
<div class="resize-handle top"></div>
<div class="resize-handle bottom"></div>
<div class="body">
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <g transform="translate(2.582 31.275) scale(0.022080 -0.022080)">
      <path d="M554 -67 497 7Q473 -4 440.5 -10.0Q408 -16 372 -16Q269 -16 195.5 33.0Q122 82 83.0 165.5Q44 249 44 354Q44 458 82.0 542.5Q120 627 193.5 676.5Q267 726 372 726Q479 726 552.0 676.5Q625 627 663.0 542.5Q701 458 701 354Q701 256 666.5 176.0Q632 96 567 46L657 -67ZM372 83Q391 83 406.5 85.0Q422 87 433 92L332 219H435L503 129Q589 201 589 354Q589 428 565.5 490.0Q542 552 494.0 589.5Q446 627 372 627Q299 627 251.0 589.5Q203 552 179.5 490.0Q156 428 156 354Q156 281 180.0 219.0Q204 157 252.0 120.0Q300 83 372 83Z" fill="currentColor"/>
    </g>
    <g transform="translate(19.032 31.275) scale(0.022080 -0.022080)">
      <path d="M86 0V710H547V611H194V395H529V299H194V0Z" fill="currentColor"/>
    </g>
    <g transform="translate(32.170 31.275) scale(0.022080 -0.022080)">
      <path d="M230 0V611H12V710H556V611H338V0Z" fill="currentColor"/>
    </g>
  </svg>
</div>
`;

  class QftGate extends HTMLElement {
    static get observedAttributes() {
      return ['span'];
    }

    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }

    connectedCallback() {
      this.#syncSpan();
    }

    attributeChangedCallback() {
      this.#syncSpan();
    }

    #syncSpan() {
      if (!this.hasAttribute('span')) {
        this.style.removeProperty('--span');
        return;
      }
      const value = Number.parseInt(this.getAttribute('span') || '1', 10);
      const span = Number.isFinite(value) && value > 0 ? value : 1;
      this.style.setProperty('--span', String(span));
    }
  }

  customElements.define('qft-gate', QftGate);
})();
