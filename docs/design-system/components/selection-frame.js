/* ─────────────────────────────────────────────────────────────────────
 * selection-frame.js — <selection-frame> カスタム要素
 *
 * 仕様: docs/design-system/selection-frame.html (正式定義)
 * 共有 CSS: docs/design-system/design-system.css の .selection-frame-host / .selection-frame ルールと
 *           1:1 で対応 (本コンポーネントは shadow DOM 内に同等の宣言を持つ)。
 *
 * 子要素を囲む形で使う:
 *   <script src="components/selection-frame.js"></script>
 *   <script src="components/h-gate.js"></script>
 *
 *   <selection-frame><h-gate></h-gate></selection-frame>              <!-- 角丸ゲート -->
 *   <selection-frame><swap-gate></swap-gate></selection-frame>        <!-- 透明ゲート -->
 *   <selection-frame shape="square">...</selection-frame>            <!-- 角ばり表示ブロック -->
 *
 * 設計判断:
 * - Shadow DOM (open) + <slot> で子要素を投影。ホスト側からは普通の wrapper として扱える
 * - :host で position: relative / isolation: isolate を当て、frame 要素を絶対配置で外側 −4 px に伸ばす
 * - 選択枠は回路上の編集対象だけを示すため、variant は持たず stroke 表現に固定する
 * - shape="square" で確率 / 振幅 / 密度行列表示ブロック用の 90 度角に切り替える
 * - Flexoki の CSS 変数 (--blue-600) は :root → shadow DOM に継承で伝播
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const TEMPLATE = `
<style>
  :host {
    position: relative;
    isolation: isolate;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .frame {
    position: absolute;
    inset: -4px;                         /* 外側への拡張 = spacing-1 = 4 px */
    border: 2px solid var(--blue-600);   /* Flexoki blue-600 / gate_selected_border */
    border-radius: 10px;                 /* 40 px ゲート本体の 6 px 角丸 + 4 px */
    pointer-events: none;
    z-index: 20;
  }
  ::slotted(*) {
    pointer-events: none;                /* 選択中の子ゲート自身の hover 枠を発火させない */
  }
  :host([shape="square"]) .frame {
    border-radius: 0;                    /* 角ばり表示ブロック: 90 度 */
  }
</style>
<div class="frame"></div>
<slot></slot>
`;

  class SelectionFrame extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('selection-frame', SelectionFrame);
})();
