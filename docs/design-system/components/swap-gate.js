/* ─────────────────────────────────────────────────────────────────────
 * swap-gate.js — <swap-gate> カスタム要素
 *
 * 仕様: docs/design-system/swap-gate.html (canonical 定義)
 *
 * 他のデザインシステムページから Swap ゲートを参照するときは、簡略マークアップ
 * (透明な四角 + × 文字) ではなく、本要素を使って canonical な見た目を
 * そのまま埋め込む。これにより swap-gate.html の仕様変更が他ページへ自動追従する。
 *
 * 使い方:
 *   <script src="components/swap-gate.js"></script>
 *
 *   <swap-gate></swap-gate>                <!-- 既定 (rest)、cyan-400 の × -->
 *   <swap-gate state="hover"></swap-gate>  <!-- ホバー枠 (stroke 派生) を強制 -->
 *   <swap-gate state="drag"></swap-gate>   <!-- drag preview (× が purple-600) -->
 *
 * 設計判断:
 * - Shadow DOM (open): ホスト側の CSS が swap-gate などを再定義しても見た目が壊れない
 * - SVG の × ラインは swap-gate.html § 03 の正規ジオメトリと完全一致
 *   (line x1=12 y1=36 x2=36 y2=12 と line x1=12 y1=12 x2=36 y2=36、stroke-width 4)
 * - Swap は「本体 fill を持たない透明なゲート」(swap-gate.html § 02) なので、
 *   ホバー枠は stroke 派生だけ持つ (内側を塗らない 2 px の紫線)。h-gate と違って
 *   .hover-gap (内側 paper 抜き) は不要 — 回路上の接続線が枠の内側を通っても消えない
 * - パレット用の surface-gap 見た目は外側 <hover-frame variant="surface-gap"> でラップする想定
 * - Flexoki の CSS 変数 (--cyan-400 / --purple-400 / --purple-600) は
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
    color: var(--cyan-400);               /* Flexoki cyan-400 / box_fill (× の stroke 色) */
  }
  :host([state="drag"]) {
    cursor: grabbing;
    color: var(--purple-600);             /* Flexoki purple-600 / drag_fill */
  }

  /* ホバー枠 (stroke 派生)。透明本体の Swap では内側を塗らず 2 px の紫線だけにする
     — 回路上の接続線が枠の内側を通っても消えないようにするため。 */
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
  <!-- × のラインは swap-gate.html § 03 の正規ジオメトリと完全一致。 -->
  <svg viewBox="0 0 48 48" aria-hidden="true">
    <line x1="12" y1="36" x2="36" y2="12" stroke="currentColor" stroke-width="4" stroke-linecap="butt"/>
    <line x1="12" y1="12" x2="36" y2="36" stroke="currentColor" stroke-width="4" stroke-linecap="butt"/>
  </svg>
</div>
`;

  class SwapGate extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('swap-gate', SwapGate);
})();
