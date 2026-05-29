/* ─────────────────────────────────────────────────────────────────────
 * circuit-dropzone.js — <circuit-dropzone> カスタム要素
 *
 * 回路の 1 セル (56px ピッチ = SLOT_SPACING / LINE_GAP) を描く。中央に 40px の
 * ゲート (GATE_SIZE) を置き、横の量子ビットワイヤと、上下方向の制御線
 * (縦コネクタ) を持ち、ゲート要素を slot に入れて使う。Qni
 * (~/Work/qni packages/elements の circuit-dropzone-element + wires.svg) の
 * 「各セルが connect-top / connect-bottom のスタブを描き、隣接セルのスタブが
 * 繋がって 1 本の制御線になる」方式を踏襲する。
 *
 * 配色は apps/web の回路描画 (src/render/circuit_connectors.rs) に合わせる:
 *   - 横ワイヤ      : Flexoki ui-2  / 2px
 *   - 縦コネクタ     : Flexoki cyan-400 / 4px (CONNECTOR_STROKE_WIDTH)
 *
 * 使い方:
 *   <script src="components/circuit-dropzone.js"></script>
 *
 *   <circuit-dropzone><h-gate></h-gate></circuit-dropzone>       <!-- ゲート + ワイヤ -->
 *   <circuit-dropzone connect-bottom><control-gate></control-gate></circuit-dropzone>
 *   <circuit-dropzone connect-top><x-gate></x-gate></circuit-dropzone>
 *   <circuit-dropzone connect-top connect-bottom></circuit-dropzone> <!-- 制御線の貫通 -->
 *
 * 反制御 (anti-control-gate) を載せるセルでは、空円の内側でワイヤ / コネクタを
 * 消し込むため、祖先で --ac-mask に紙色 (var(--bg)) を渡す
 * (例: 回路コンテナに `--ac-mask: var(--bg)`)。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const TEMPLATE = `
<style>
  :host {
    display: block;
    width: 56px;                          /* SLOT_SPACING = spacing-14 (列ピッチ) */
    height: 56px;                         /* LINE_GAP = spacing-14 (ワイヤ間隔) */
    position: relative;
  }
  svg.wires {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    pointer-events: none;
    overflow: visible;
  }
  .wire {
    stroke: var(--ui-2);                  /* Flexoki ui-2 = ワイヤ */
    stroke-width: 2;
  }
  #connect-top,
  #connect-bottom {
    stroke: var(--cyan-400);              /* Flexoki cyan-400 = 制御線 */
    stroke-width: 4;                      /* CONNECTOR_STROKE_WIDTH */
    display: none;
  }
  :host([connect-top]) #connect-top { display: inline; }
  :host([connect-bottom]) #connect-bottom { display: inline; }

  /* 40px のゲートを 56px セルの中央へ置き、ワイヤ / コネクタの上に重ねる
     (GATE_SIZE 40 < SLOT_SPACING 56 なので上下左右に 8px の余白ができる)。 */
  ::slotted(*) {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 10;
  }
</style>
<svg class="wires" viewBox="0 0 48 48" preserveAspectRatio="none" aria-hidden="true">
  <line class="wire" x1="0" y1="24" x2="48" y2="24" vector-effect="non-scaling-stroke"></line>
  <line id="connect-top" x1="24" y1="0" x2="24" y2="24" vector-effect="non-scaling-stroke"></line>
  <line id="connect-bottom" x1="24" y1="24" x2="24" y2="48" vector-effect="non-scaling-stroke"></line>
</svg>
<slot></slot>
`;

  class CircuitDropzone extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('circuit-dropzone', CircuitDropzone);
})();
