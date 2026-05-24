/* ─────────────────────────────────────────────────────────────────────
 * p-gate.js — <p-gate> カスタム要素
 *
 * 仕様: docs/design-system/p-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから P を参照するときは、inline SVG ではなく、
 * 本要素を使って canonical な見た目をそのまま埋め込む。
 *
 * 使い方:
 *   <script src="components/p-gate.js"></script>
 *
 *   <p-gate></p-gate>                <!-- 既定 (rest) -->
 *   <p-gate state="hover"></p-gate>  <!-- 共通の角丸ホバー枠 + 背景の空きを強制 -->
 *   <p-gate state="drag"></p-gate>   <!-- drag preview (本体 purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が `.p-gate` などを再定義しても見た目が壊れない
 * - SVG path は実装 (apps/web/assets/icons/p.svg) と一致する Geist 700 outline
 * - 本体は真円だが、ホバー枠は実装に合わせて共通の角丸枠を使う
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
    border-radius: 50%;         /* 真円 */
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
  <!-- P の glyph。path d は apps/web/assets/icons/p.svg と完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
<g transform="translate(13.733 34.565) scale(0.029760 -0.029760)">
    <path d="M92 0V710H347Q466 710 532.0 653.0Q598 596 598 495Q598 392 532.0 334.0Q466 276 347 276H178V0ZM178 360H347Q508 360 508 495Q508 626 347 626H178Z" fill="currentColor"/>
  </g>
  </svg>
</div>
`;

  class PGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('p-gate', PGate);
})();
