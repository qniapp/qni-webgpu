/* ─────────────────────────────────────────────────────────────────────
 * x-gate.js — <x-gate> カスタム要素
 *
 * 仕様: docs/design-system/x-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから X ゲートを参照するときは、簡略マークアップ
 * (色付き円 + Plus グリフ) ではなく、本要素を使って canonical な見た目を
 * そのまま埋め込む。これにより x-gate.html の仕様変更が他ページへ自動追従する。
 *
 * 使い方:
 *   <script src="components/x-gate.js"></script>
 *
 *   <x-gate></x-gate>                <!-- 既定 (rest)、cyan-400 真円 + paper Plus -->
 *   <x-gate state="hover"></x-gate>  <!-- ホバー枠 (purple-400 同心円) を強制 -->
 *   <x-gate state="drag"></x-gate>   <!-- drag preview (本体 purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が .x-gate などを再定義しても見た目が壊れない
 * - X (Pauli-X / NOT) ゲートは Quirk 慣例に従い真円本体に Plus グリフ。
 *   H / Y / Z などの角丸正方形 (border-radius 6 px) とは別形状で、グリフを覚えなくても
 *   形だけで「制御先 / Pauli-X」が判別できる
 * - SVG Plus path は apps/web/assets/icons/plus.svg と完全一致
 * - 本体が真円なのでホバー枠も同心円 (border-radius: 50%)。h-gate のような角丸 6 + 4 = 10
 *   の関係式は X には適用されない (常に真円)
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

  /* ホバー枠 (purple-400 同心円) + 背景の空き (paper)。
     真円本体に対し同心円で枠を描く (border-radius: 50%)。 */
  .hover-frame,
  .hover-gap {
    position: absolute;
    display: none;
    pointer-events: none;
    border-radius: 50%;
  }
  .hover-frame {
    inset: -4px;                          /* body_rect.expand(4.0) */
    background: var(--purple-400);        /* Flexoki purple-400 / gate_hover_border */
  }
  .hover-gap {
    inset: -2px;                          /* 2 px の paper 帯 */
    background: var(--bg);                /* Flexoki bg / paper */
  }
  :host([state="hover"]) .hover-frame,
  :host(:not([state]):hover) .hover-frame { display: block; }
  :host([state="hover"]) .hover-gap,
  :host(:not([state]):hover) .hover-gap { display: block; }

  /* 本体: 真円 (Quirk Pauli-X / NOT 慣例) + paper Plus グリフ。 */
  .body {
    position: absolute;
    inset: 0;
    background: var(--cyan-400);          /* Flexoki cyan-400 / box_fill */
    border-radius: 50%;                   /* 真円: H/Y/Z の rounded-md (6 px) と対比 */
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
  <!-- Plus グリフ。path d / transform は apps/web/assets/icons/plus.svg と完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <g transform="translate(15.637 32.690) scale(0.029760 -0.029760)" fill="currentColor">
      <path d="M235 51V246H40V338H235V533H327V338H522V246H327V51Z"/>
    </g>
  </svg>
</div>
`;

  class XGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('x-gate', XGate);
})();
