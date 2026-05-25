/* ─────────────────────────────────────────────────────────────────────
 * p-gate.js — <p-gate> カスタム要素
 *
 * 仕様: docs/design-system/p-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから Phase ゲートを参照するときは、inline SVG ではなく、
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
 * - Φ グリフは Qni の phase-gate.svg と同じ円 + 斜線の字形
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
  <!-- Φ グリフ。Qni packages/elements/icon/phase-gate.svg と同じ円 + 斜線。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <path d="M18.2857 36L29.7143 12M16 24.5714C16 25.622 16.2069 26.6623 16.609 27.6329C17.011 28.6035 17.6003 29.4854 18.3431 30.2283C19.086 30.9712 19.9679 31.5604 20.9385 31.9625C21.9091 32.3645 22.9494 32.5714 24 32.5714C25.0506 32.5714 26.0909 32.3645 27.0615 31.9625C28.0321 31.5604 28.914 30.9712 29.6569 30.2283C30.3997 29.4854 30.989 28.6035 31.391 27.6329C31.7931 26.6623 32 25.622 32 24.5714C32 23.5209 31.7931 22.4806 31.391 21.51C30.989 20.5394 30.3997 19.6574 29.6569 18.9146C28.914 18.1717 28.0321 17.5824 27.0615 17.1804C26.0909 16.7784 25.0506 16.5714 24 16.5714C22.9494 16.5714 21.9091 16.7784 20.9385 17.1804C19.9679 17.5824 19.086 18.1717 18.3431 18.9146C17.6003 19.6574 17.011 20.5394 16.609 21.51C16.2069 22.4806 16 23.5209 16 24.5714Z" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
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
