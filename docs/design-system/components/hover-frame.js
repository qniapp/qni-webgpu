/* ─────────────────────────────────────────────────────────────────────
 * hover-frame.js — <hover-frame> カスタム要素
 *
 * 仕様: docs/design-system/hover-frame.html (canonical 定義)
 * 共有 CSS: docs/design-system/design-system.css の .hover-frame-host / .hover-frame ルールと
 *           1:1 で対応 (本コンポーネントは shadow DOM 内に同等の宣言を持つ)。
 *
 * 子要素を囲む形で使う:
 *   <script src="components/hover-frame.js"></script>
 *   <script src="components/h-gate.js"></script>
 *
 *   <hover-frame variant="stroke"><h-gate></h-gate></hover-frame>       <!-- 回路上 (線だけの紫枠) -->
 *   <hover-frame variant="surface-gap"><h-gate></h-gate></hover-frame>  <!-- パレット (内側 bg 抜き) -->
 *
 * 設計判断:
 * - Shadow DOM (open) + <slot> で子要素を投影。ホスト側からは普通の wrapper として扱える
 * - :host で position: relative / isolation: isolate を当て、frame 要素を絶対配置で外側 −4 px に伸ばす
 * - variant 属性で 2 種類の見た目 (stroke / surface-gap) を切り替え
 *   - stroke: 2 px 線だけ (透明なゲートで接続線を消さない用)
 *   - surface-gap: 4 px 紫枠 + 内側 2 px の paper 抜き (パレット用)
 * - Flexoki の CSS 変数 (--purple-400 / --bg) は :root → shadow DOM に inheritance で伝播
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
    inset: -4px;                           /* 外側への拡張 = spacing-1 = 4 px */
    border-radius: 10px;                   /* 40 px ゲート本体の 6 px 角丸 + 4 px */
    pointer-events: none;
  }
  :host([variant="stroke"]) .frame {
    border: 2px solid var(--purple-400);   /* Flexoki purple-400 / gate_hover_border */
  }
  :host([variant="surface-gap"]) .frame {
    background: var(--purple-400);
    z-index: -2;
  }
  :host([variant="surface-gap"]) .frame::after {
    content: '';
    position: absolute;
    inset: 2px;                            /* 内側 2 px を paper で抜く */
    border-radius: 8px;
    background: var(--bg);                 /* Flexoki bg / paper */
  }
</style>
<div class="frame"></div>
<slot></slot>
`;

  class HoverFrame extends HTMLElement {
    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).innerHTML = TEMPLATE;
    }
  }

  customElements.define('hover-frame', HoverFrame);
})();
