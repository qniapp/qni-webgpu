/* ─────────────────────────────────────────────────────────────────────
 * resize-handle.js — リサイズハンドル primitive の共有 DOM 生成補助
 *
 * 仕様: docs/design-system/resize-handle.html
 * CSS:  docs/design-system/design-system.css (.resize-handle / .resize-handle--top / .resize-handle--bottom)
 *
 * 注記: この補助の使い方は本ファイルに集約し、デザイン仕様本文には載せない。
 *
 * 使い方:
 *   <link rel="stylesheet" href="design-system.css" />  // .resize-handle ルールを供給
 *   <script src="resize-handle.js"></script>            // window.attachResizeHandles を生やす
 *
 *   const host = document.querySelector('.ad-host');     // 任意のホスト
 *   host.classList.add('resize-handle-host');            // hover で visible になる契約
 *   attachResizeHandles(host);
 *
 * 仕様ページの常時表示モック (ホバーしなくても状態を強制):
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
   * @param {'visible'|'hover'|'active'} [opts.handleState] - 強制状態 (仕様ページのモック用)
   * @returns {{ top: HTMLDivElement, bottom: HTMLDivElement }}
   */
  function attachResizeHandles(host, opts) {
    opts = opts || {};
    const width = 24;
    const top = createHandle('top', width, opts.handleState);
    // bottom 側は何かの状態が指定されていれば常に visible で表示する (仕様ページのモック慣習)
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
