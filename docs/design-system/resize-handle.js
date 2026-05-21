/* ─────────────────────────────────────────────────────────────────────
 * resize-handle.js — リサイズハンドル primitive の共有 DOM 生成ヘルパ
 *
 * 仕様: docs/design-system/resize-handle.html
 * CSS:  docs/design-system/design-system.css (.resize-handle / .resize-handle--top / .resize-handle--bottom)
 *
 * 使い方:
 *   <link rel="stylesheet" href="design-system.css" />  // .resize-handle ルールを供給
 *   <script src="resize-handle.js"></script>            // window.attachResizeHandles を生やす
 *
 *   const host = document.querySelector('.cd-host');     // 任意のホスト
 *   host.classList.add('resize-handle-host');            // hover で visible になる契約
 *   attachResizeHandles(host, { widthMode: 'fixed' });   // probability は固定 24 px
 *
 *   const host2 = document.querySelector('.ad-host');
 *   host2.classList.add('resize-handle-host');
 *   attachResizeHandles(host2, { widthMode: 'canvas60' }); // amplitude / density は canvas 幅 × 0.6
 *
 * spec ページの常時表示モック (ホバーしなくても状態を強制):
 *   attachResizeHandles(host, { handleState: 'visible' });  // 両方 visible
 *   attachResizeHandles(host, { handleState: 'hover' });    // top: hover、bottom: visible
 *   attachResizeHandles(host, { handleState: 'active' });   // top: active、bottom: visible
 *
 * 静的プレビュー (パレットアイコン等) ではハンドル不要なので呼ばない。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  /**
   * @param {HTMLElement} host - ハンドルを appendChild する親要素
   * @param {Object} [opts]
   * @param {'fixed'|'canvas60'} [opts.widthMode='fixed'] - 幅算出ポリシー
   * @param {'visible'|'hover'|'active'} [opts.handleState] - 強制 state (spec モック用)
   * @param {number} [opts.canvasWidth] - canvas 幅を明示 (省略時は host 内の <canvas>.width を使う)
   * @returns {{ top: HTMLDivElement, bottom: HTMLDivElement }}
   */
  function attachResizeHandles(host, opts) {
    opts = opts || {};
    const widthMode = opts.widthMode || 'fixed';
    let width = 24;
    if (widthMode === 'canvas60') {
      // canvas 幅を取得。明示指定 > host 内の <canvas>.width > fallback 24
      const canvasWidth =
        opts.canvasWidth != null
          ? opts.canvasWidth
          : (host.querySelector('canvas') || {}).width || 24;
      width = Math.max(24, Math.round(canvasWidth * 0.6));
    }
    const top = createHandle('top', width, opts.handleState);
    // bottom 側は何かの state が指定されていれば常に visible で表示する (spec モック慣習)
    const bottomState = opts.handleState ? 'visible' : undefined;
    const bottom = createHandle('bottom', width, bottomState);
    host.appendChild(top);
    host.appendChild(bottom);
    return { top, bottom };
  }

  function createHandle(position, width, state) {
    const el = document.createElement('div');
    el.className = 'resize-handle resize-handle--' + position;
    el.style.width = width + 'px';
    el.setAttribute('aria-hidden', 'true');
    if (state) el.dataset.handleState = state;
    return el;
  }

  window.attachResizeHandles = attachResizeHandles;
})();
