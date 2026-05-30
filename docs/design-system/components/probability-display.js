/* ─────────────────────────────────────────────────────────────────────
 * probability-display.js — <probability-display> カスタム要素
 *
 * 確率表示ブロックの正規描画を 1 箇所へ集約する。height / probs / canvas /
 * placeholder / icon / state / handle-state 属性で仕様ページの各状態を表す。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const CANVAS_COLORS = Object.freeze({
    bg: '#FFFCF0',        // Flexoki bg
    ui2: '#DAD8CE',       // Flexoki ui-2
    blue200: '#92BFDB',   // Flexoki blue-200
    blue400: '#4385BE',   // Flexoki blue-400
    tx3: '#B7B5AC',       // Flexoki tx-3
    purple100: '#ECE1F3', // Flexoki purple-100
  });

  const TEMPLATE = document.createElement('template');
  TEMPLATE.innerHTML = `
<style>
  :host {
    display: inline-block;
    position: relative;
    width: 40px;
    flex-shrink: 0;
  }
  .cd {
    width: 40px;
    background: var(--bg);
    position: relative;
    overflow: visible;
    flex-shrink: 0;
  }
  .cd::after {
    content: '';
    position: absolute;
    inset: 0;
    border: 1px solid var(--ui-2);
    pointer-events: none;
  }
  .cd .row {
    position: absolute;
    left: 0;
    right: 0;
    overflow: hidden;
  }
  .cd .row .bar {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: var(--blue-200);          /* Flexoki blue-200 / state_fill */
    border-right: 1px solid var(--blue-400);
  }
  .cd .row .label {
    position: absolute;
    right: 2px;
    top: 50%;
    transform: translateY(-50%);
    font-family: var(--font-mono);
    font-size: 12px;                      /* text-xs */
    font-weight: 300;
    color: var(--tx);
    pointer-events: none;
    letter-spacing: -0.02em;
    white-space: nowrap;
  }
  .cd .row .label .dot {
    display: inline-block;
    margin-left: -3px;
    margin-right: -3px;
  }
  .cd .row .label .pct {
    font-size: 12px;
    vertical-align: baseline;
  }
  .cd .div {
    position: absolute;
    right: 0;
    height: 1px;
    background: var(--ui-2);
    pointer-events: none;
  }
  .cd .log-hint-svg {
    position: absolute;
    top: 0;
    left: 0;
    pointer-events: none;
    z-index: 1;
  }
  .cd .row .bar-clip {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    overflow: hidden;
    pointer-events: none;
  }
  .cd .row .bar-clip .row-extent {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 40px;
  }
  .cd .row .bar-clip .label { color: var(--tx); }
  .cd:hover,
  :host([state="hover"]) .cd {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--purple-400);
  }
  .cd .row:hover::after {
    content: '';
    position: absolute;
    inset: 0;
    border: 2px solid var(--purple-400);
    pointer-events: none;
    z-index: 10;
  }
  .cd.icon .row { pointer-events: none; }

  .cd.cd--placeholder { background: var(--purple-100); }
  .cd.cd--placeholder .cd-ph-row {
    position: absolute;
    left: 0;
    right: 0;
    height: 1px;
    background: var(--ui-2);
    pointer-events: none;
  }

  canvas.cd-canvas {
    display: block;
    flex-shrink: 0;
    background: var(--bg);
  }
  canvas.cd-canvas:hover,
  :host([state="hover"]) canvas.cd-canvas {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--purple-400);
  }

  .cd-popup {
    position: absolute;
    display: none;
    pointer-events: none;
    z-index: 50;
    background: var(--bg);
    border: 1px solid var(--ui-2);
    border-radius: 6px;
    padding: 12px 16px;
    font-family: 'Geist', sans-serif;
    font-size: 12px;
    line-height: 1.4;
    color: var(--tx);
    white-space: nowrap;
    box-shadow: 0 4px 12px rgba(16, 15, 15, 0.06); /* Flexoki black 6% */
  }
  .cd-popup .cd-popup-ket {
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 400;
    letter-spacing: 0.02em;
    color: var(--tx);
    text-align: center;
  }
  .cd-popup .cd-popup-sub {
    font-size: 12px;
    margin-top: 4px;
    color: var(--tx-2);
  }
  .cd-popup .cd-popup-divider {
    height: 1px;
    background: var(--ui-2);
    margin: 12px 0;
  }
  .cd-popup .cd-popup-row {
    display: flex;
    gap: 16px;
    align-items: baseline;
    font-family: var(--font-mono);
  }
  .cd-popup .cd-popup-row + .cd-popup-row { margin-top: 4px; }
  .cd-popup .cd-popup-label {
    font-size: 12px;
    color: var(--tx-2);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    min-width: 32px;
  }
  .cd-popup .cd-popup-value {
    font-size: 14px;
    color: var(--tx);
    font-variant-numeric: tabular-nums;
    margin-left: auto;
  }

  /* Shadow DOM 内では design-system.css の .resize-handle が届かないため、
     resize-handle.html の共通仕様を本要素内へ写す。変更時は同仕様と同期する。 */
  .resize-handle {
    position: absolute;
    left: 50%;
    width: 24px;
    height: 6px;
    background: var(--cyan-400);
    border-radius: 3px;
    opacity: 0;
    cursor: ns-resize;
    pointer-events: auto;
    z-index: 20;
    transform: translateX(-50%) scale(0.7);
    transition:
      opacity 200ms ease-out,
      transform 250ms cubic-bezier(0.34, 1.56, 0.64, 1),
      background 120ms ease-out;
  }
  .resize-handle--top { top: -12px; }
  .resize-handle--bottom { bottom: -12px; }
  :host(:hover) .resize-handle,
  .resize-handle[data-handle-state="visible"],
  .resize-handle[data-handle-state="hover"],
  .resize-handle[data-handle-state="active"] {
    opacity: 1;
    transform: translateX(-50%) scale(1);
  }
  .resize-handle:hover,
  .resize-handle[data-handle-state="hover"] {
    background: var(--cyan-600);
    transform: translateX(-50%) scale(1.15);
    transition: none;
  }
  .resize-handle:active,
  .resize-handle[data-handle-state="active"] {
    background: var(--cyan-600);
    transform: translateX(-50%) scale(1.25);
    transition: none;
  }
</style>
<div class="render-root"></div>
`;

  class ProbabilityDisplay extends HTMLElement {
    static get observedAttributes() {
      return ['height', 'probs', 'canvas', 'pattern', 'n', 'span', 'placeholder', 'icon', 'no-popup', 'handle-state', 'state'];
    }

    constructor() {
      super();
      this.attachShadow({ mode: 'open' }).appendChild(TEMPLATE.content.cloneNode(true));
    }

    connectedCallback() {
      this.#render();
    }

    attributeChangedCallback() {
      if (this.isConnected) this.#render();
    }

    #render() {
      const root = this.shadowRoot.querySelector('.render-root');
      root.innerHTML = '';
      const totalH = Number(this.getAttribute('height') || '40');
      const opts = {
        icon: this.hasAttribute('icon'),
        placeholder: this.hasAttribute('placeholder'),
        noPopup: this.hasAttribute('no-popup'),
        handleState: this.getAttribute('handle-state'),
      };
      const probs = this.#probabilities(opts.placeholder);
      const useCanvas = this.hasAttribute('canvas');
      const visual = useCanvas
        ? this.#buildCanvas(probs, totalH, opts)
        : this.#buildDom(probs, totalH, opts);
      root.appendChild(visual);
      if (!opts.icon) this.#attachResizeHandles(root, opts.handleState);
      if (!opts.icon && !opts.noPopup && !opts.placeholder) this.#attachPopup(root, visual, probs, totalH);
    }

    #probabilities(placeholder) {
      const probsAttr = this.getAttribute('probs');
      const n = this.getAttribute('n')
        ? Number(this.getAttribute('n'))
        : probsAttr
          ? probsAttr.split(',').length
          : 1 << Number(this.getAttribute('span') || 1);
      if (placeholder) return new Array(n).fill(0);
      const pattern = this.getAttribute('pattern');
      if (pattern) return Array.from(this.#genPattern(pattern, n));
      return probsAttr ? probsAttr.split(',').map(Number) : new Array(n).fill(0);
    }

    #buildLabel(p) {
      const [intPart, decPart] = p.toFixed(1).split('.');
      return `${intPart}<span class="dot">.</span>${decPart}<span class="pct">%</span>`;
    }

    #buildDom(probs, totalH, opts) {
      const n = probs.length;
      const rowH = totalH / n;
      const cd = document.createElement('div');
      cd.className = `cd${opts.placeholder ? ' cd--placeholder' : ''}${opts.icon ? ' icon' : ''}`;
      cd.style.height = `${totalH}px`;

      if (opts.placeholder) {
        if (rowH > 2) {
          for (let i = 1; i < n; i++) {
            const row = document.createElement('div');
            row.className = 'cd-ph-row';
            row.style.top = `${(i * rowH).toFixed(3)}px`;
            cd.appendChild(row);
          }
        }
        return cd;
      }

      const showText = !opts.icon && n <= 16 && rowH > 8;
      const showLogHints = !opts.icon && !showText;
      const width = 40;
      if (showLogHints) cd.appendChild(this.#buildLogHintSvg(probs, totalH, rowH, width));

      probs.forEach((p, i) => {
        const row = document.createElement('div');
        row.className = 'row';
        row.style.top = `${i * rowH}px`;
        row.style.height = `${rowH}px`;

        const bar = document.createElement('div');
        bar.className = 'bar';
        bar.style.width = `${p}%`;
        row.appendChild(bar);

        if (showText) {
          const labelHtml = this.#buildLabel(p);
          const label = document.createElement('div');
          label.className = 'label';
          label.innerHTML = labelHtml;
          row.appendChild(label);

          const clip = document.createElement('div');
          clip.className = 'bar-clip';
          clip.style.width = `${p}%`;
          const extent = document.createElement('div');
          extent.className = 'row-extent';
          const clippedLabel = document.createElement('div');
          clippedLabel.className = 'label';
          clippedLabel.innerHTML = labelHtml;
          extent.appendChild(clippedLabel);
          clip.appendChild(extent);
          row.appendChild(clip);
        }
        cd.appendChild(row);
      });

      for (let i = 1; i < n; i++) {
        const divider = document.createElement('div');
        divider.className = 'div';
        divider.style.top = `${i * rowH}px`;
        divider.style.left = `${Math.max(probs[i - 1], probs[i])}%`;
        cd.appendChild(divider);
      }
      return cd;
    }

    #buildLogHintSvg(probs, totalH, rowH, width) {
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('class', 'log-hint-svg');
      svg.setAttribute('width', String(width));
      svg.setAttribute('height', String(totalH));
      svg.setAttribute('viewBox', `0 0 ${width} ${totalH}`);
      svg.setAttribute('preserveAspectRatio', 'none');
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      const span = Math.log2(probs.length);
      const s = 1 / (4 + Math.max(8, span));
      const e = Math.max(rowH, 1);
      // log(0) is undefined: an impossible outcome (prob 0) has no log-scale
      // position, so it gets neither a tick nor a connector. The hint is a set of
      // vertical ticks at the rows that carry probability, joined by a horizontal
      // connector only where two consecutive rows are both non-zero — so a full
      // bar leaves no full-width connector down to the empty rows below it.
      let d = '';
      let prevPx = null;
      for (let i = 0; i < probs.length; i++) {
        const p01 = probs[i] / 100;
        if (p01 <= 0) {
          prevPx = null;
          continue;
        }
        const xRatio = Math.min(1, Math.max(0, 1 + Math.log(p01) * s));
        const px = width * xRatio;
        const pyTop = rowH * i;
        const pyEnd = rowH * i + e;
        if (prevPx !== null) {
          d += ` M ${prevPx.toFixed(3)} ${pyTop.toFixed(3)} L ${px.toFixed(3)} ${pyTop.toFixed(3)}`;
        }
        d += ` M ${px.toFixed(3)} ${pyTop.toFixed(3)} L ${px.toFixed(3)} ${pyEnd.toFixed(3)}`;
        prevPx = px;
      }
      path.setAttribute('d', d.trim());
      path.setAttribute('stroke', 'var(--tx-3)');
      path.setAttribute('stroke-width', '1');
      path.setAttribute('fill', 'none');
      path.setAttribute('shape-rendering', 'crispEdges');
      svg.appendChild(path);
      return svg;
    }

    #buildCanvas(probs, totalH, opts) {
      const n = probs.length;
      const rowH = totalH / n;
      const w = 40;
      const h = totalH;
      const dpr = window.devicePixelRatio || 1;
      const canvas = document.createElement('canvas');
      canvas.className = 'cd-canvas';
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      canvas.style.width = `${w}px`;
      canvas.style.height = `${h}px`;
      const ctx = canvas.getContext('2d');
      ctx.scale(dpr, dpr);

      if (opts.placeholder) {
        ctx.fillStyle = CANVAS_COLORS.purple100;
        ctx.fillRect(0, 0, w, h);
        if (rowH > 2) {
          ctx.strokeStyle = CANVAS_COLORS.ui2;
          ctx.lineWidth = 1;
          ctx.beginPath();
          for (let i = 1; i < n; i++) {
            const y = Math.round(rowH * i) + 0.5;
            ctx.moveTo(0, y);
            ctx.lineTo(w, y);
          }
          ctx.stroke();
        }
        ctx.strokeStyle = CANVAS_COLORS.ui2;
        ctx.lineWidth = 1;
        ctx.strokeRect(0.5, 0.5, w - 1, h - 1);
        return canvas;
      }

      ctx.fillStyle = CANVAS_COLORS.bg;
      ctx.fillRect(0, 0, w, h);
      const showText = !opts.icon && n <= 16 && rowH > 8;
      const showLogHints = !opts.icon && !showText;
      const e = Math.max(rowH, 1);

      if (showLogHints) {
        const span = Math.log2(n);
        const s = 1 / (4 + Math.max(8, span));
        ctx.strokeStyle = CANVAS_COLORS.tx3;
        ctx.lineWidth = 1;
        ctx.beginPath();
        // log(0) is undefined: skip impossible outcomes (prob 0) entirely so a
        // full bar leaves no full-width connector down to the empty rows below.
        let prevPx = null;
        for (let i = 0; i < n; i++) {
          const p01 = probs[i] / 100;
          if (p01 <= 0) {
            prevPx = null;
            continue;
          }
          const px = w * Math.min(1, Math.max(0, 1 + Math.log(p01) * s));
          const py = rowH * i;
          if (prevPx !== null) {
            ctx.moveTo(prevPx, py);
            ctx.lineTo(px, py);
          }
          ctx.moveTo(px, py);
          ctx.lineTo(px, py + e);
          prevPx = px;
        }
        ctx.stroke();
      }

      ctx.fillStyle = CANVAS_COLORS.blue200;
      for (let i = 0; i < n; i++) {
        ctx.fillRect(0, rowH * i, (probs[i] / 100) * w, e);
      }
      ctx.strokeStyle = CANVAS_COLORS.blue400;
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (let i = 0; i < n; i++) {
        const bw = (probs[i] / 100) * w;
        if (bw <= 0) continue;
        const x = Math.round(bw) - 0.5;
        const py = rowH * i;
        ctx.moveTo(x, py);
        ctx.lineTo(x, py + e);
      }
      ctx.stroke();

      if (rowH > 2) {
        ctx.strokeStyle = CANVAS_COLORS.ui2;
        ctx.lineWidth = 1;
        ctx.beginPath();
        for (let i = 1; i < n; i++) {
          const start = (Math.max(probs[i - 1], probs[i]) / 100) * w;
          const y = Math.round(rowH * i) + 0.5;
          ctx.moveTo(start, y);
          ctx.lineTo(w, y);
        }
        ctx.stroke();
      }

      ctx.strokeStyle = CANVAS_COLORS.ui2;
      ctx.lineWidth = 1;
      ctx.strokeRect(0.5, 0.5, w - 1, h - 1);
      return canvas;
    }

    #attachResizeHandles(root, handleState) {
      const top = document.createElement('div');
      top.className = 'resize-handle resize-handle--top';
      top.setAttribute('aria-hidden', 'true');
      if (handleState) top.dataset.handleState = handleState;
      const bottom = document.createElement('div');
      bottom.className = 'resize-handle resize-handle--bottom';
      bottom.setAttribute('aria-hidden', 'true');
      if (handleState) bottom.dataset.handleState = 'visible';
      root.appendChild(top);
      root.appendChild(bottom);
    }

    #attachPopup(root, visual, probs, totalH) {
      const rowH = totalH / probs.length;
      const span = Math.log2(probs.length);
      const popup = document.createElement('div');
      popup.className = 'cd-popup';
      root.appendChild(popup);
      visual.addEventListener('mousemove', (event) => {
        const rect = visual.getBoundingClientRect();
        const idx = Math.floor((event.clientY - rect.top) / rowH);
        if (idx < 0 || idx >= probs.length) {
          popup.style.display = 'none';
          return;
        }
        popup.innerHTML = this.#popupHtml(idx, probs[idx], span);
        popup.style.left = '48px';
        popup.style.top = `${idx * rowH}px`;
        popup.style.display = 'block';
      });
      visual.addEventListener('mouseleave', () => {
        popup.style.display = 'none';
      });
    }

    #popupHtml(idx, p100, span) {
      const bitWidth = Math.max(1, span);
      const ket = idx.toString(2).padStart(bitWidth, '0');
      const rawPct = `${p100.toFixed(4)}%`;
      const dbValue = p100 > 0
        ? `${(Math.log10(p100 / 100) * 10).toFixed(1)} dB`
        : '−∞ dB';
      const dbDisplay = dbValue.replace(/^-/, '−');
      return `
        <div class="cd-popup-ket">|${ket}⟩</div>
        <div class="cd-popup-sub">Probability if measured · k = ${idx}</div>
        <div class="cd-popup-divider"></div>
        <div class="cd-popup-row">
          <span class="cd-popup-label">raw</span>
          <span class="cd-popup-value">${rawPct}</span>
        </div>
        <div class="cd-popup-row">
          <span class="cd-popup-label">log</span>
          <span class="cd-popup-value">${dbDisplay}</span>
        </div>
      `;
    }

    #genPattern(name, n) {
      if (name === 'gaussian') {
        const center = n / 2;
        const sigma = n / 16;
        const out = new Float32Array(n);
        for (let k = 0; k < n; k++) {
          const d = (k - center) / sigma;
          out[k] = 80 * Math.exp(-(d * d) / 2);
        }
        return out;
      }
      return new Float32Array(n);
    }
  }

  customElements.define('probability-display', ProbabilityDisplay);
})();
