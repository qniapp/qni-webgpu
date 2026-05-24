/* ─────────────────────────────────────────────────────────────────────
 * h-gate.js — <h-gate> カスタム要素
 *
 * 仕様: docs/design-system/gate-chrome.html (共通クロム) +
 *       docs/design-system/h-gate.html (H 固有の字形 / 意味論)
 *
 * 他のデザインシステムページから H ゲートを参照するときは、簡略マークアップ
 * (色付き四角 + テキストラベル) ではなく、本要素を使って正規の見た目を
 * そのまま埋め込む。これにより gate-chrome.html / h-gate.html に沿った本要素の更新が他ページへ自動追従する。
 *
 * 使い方:
 *   <script src="components/h-gate.js"></script>
 *
 *   <h-gate></h-gate>                <!-- 既定 (rest) -->
 *   <h-gate state="hover"></h-gate>  <!-- ホバー枠 + 背景の空きを強制 -->
 *   <h-gate state="drag"></h-gate>   <!-- drag preview (本体 purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が `.h-gate` などを再定義しても見た目が壊れない
 * - SVG path は実装 (apps/web/assets/icons/h.svg) と一致する Geist 700 outline
 * - Flexoki の CSS 変数 (--cyan-400 / --bg / --purple-400 / --purple-600) は
 *   :root から Shadow DOM へ inheritance で伝播するため、Shadow root 内で var() で参照できる
 * - ホバー枠は ::before / ::after の代わりに <div class="hover-frame"> / <div class="hover-gap">
 *   を使い、source order と body の不透明背景で「枠 + 空き + 本体」の 3 層を構成する
 *   (z-index を使わずに済むため、ホスト側の stacking context 干渉を受けない)
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

  /* ホバー枠 (purple-400) + 背景の空き (paper)。
     source order が body より先なので、body (本体) の背景で内側を隠し、
     枠と空きの「ring」だけが host の外側に visible になる。 */
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

  /* 本体 (cyan-400) + paper グリフ。共通クロムは gate-chrome.html、H 字形は h-gate.html と一致。 */
  .body {
    position: absolute;
    inset: 0;
    background: var(--cyan-400);          /* Flexoki cyan-400 / box_fill */
    border-radius: 6px;                   /* rounded-md = CornerRadius::same(6) */
    color: var(--bg);                     /* paper グリフ (SVG path の currentColor) */
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
  <!-- Geist 700 の "H" outline。path d は apps/web/assets/icons/h.svg と完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <g transform="translate(13.391 34.565) scale(0.029760 -0.029760)">
      <path d="M92 0V710H178V399H535V710H621V0H535V315H178V0Z" fill="currentColor"/>
    </g>
  </svg>
</div>
`;

  class HGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('h-gate', HGate);
})();
