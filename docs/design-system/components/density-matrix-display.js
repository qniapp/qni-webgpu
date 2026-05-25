/* ─────────────────────────────────────────────────────────────────────
 * density-matrix-display.js — <density-matrix-display> カスタム要素
 *
 * 密度行列表示ブロックのデザインシステム用 canvas 描画を 1 箇所へ集約する。
 * span / state / placeholder / icon / no-popup / handle-state 属性で仕様ページの
 * 各状態を表す。実アプリの量子状態計算ではなく、仕様ページの静的見本用データ
 * だけを生成する。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const ELEMENT_NAME = 'density-matrix-display';
  if (customElements.get(ELEMENT_NAME)) return;

  const CANVAS_COLORS = Object.freeze({
    bg: '#FFFCF0',        // Flexoki bg
    blue200: '#92BFDB',   // Flexoki blue-200 / state_fill
    blue400: '#4385BE',   // Flexoki blue-400 / popup_icon
    ui2: '#DAD8CE',       // Flexoki ui-2 / line
    tx2: '#6F6E69',       // Flexoki tx-2 / state_outline
    tx: '#100F0F',        // Flexoki tx / state_needle
    purple100: '#ECE1F3', // Flexoki purple-100 / placeholder
  });

  const TEMPLATE = document.createElement('template');
  TEMPLATE.innerHTML = `
<style>
  :host {
    display: inline-block;
    position: relative;
    flex-shrink: 0;
    line-height: 0;
  }
  .render-root {
    display: inline-flex;
    align-items: stretch;
  }
  canvas.dm-canvas {
    display: block;
    cursor: crosshair;
    flex-shrink: 0;
  }
  :host([icon]) canvas.dm-canvas {
    cursor: default;
  }
  canvas.dm-canvas:hover,
  :host([visual-state="hover"]) canvas.dm-canvas {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--purple-400);
  }

  .dm-popup {
    position: absolute;
    display: none;
    pointer-events: none;
    z-index: 50;
    background: var(--bg);
    border: 1px solid var(--ui-2);
    border-radius: 6px;
    padding: 12px 16px;                    /* spacing-3 / spacing-4 */
    font-family: var(--font-body);
    font-size: 12px;                       /* text-xs */
    line-height: 16px;                     /* text-xs line-height */
    color: var(--tx);
    white-space: nowrap;
    box-shadow: 0 4px 12px rgba(16, 15, 15, 0.06); /* Flexoki black 6% */
  }
  .dm-popup .dm-popup-ket {
    font-family: var(--font-mono);
    font-size: 14px;                       /* text-sm */
    font-weight: 400;
    letter-spacing: 0.02em;
    color: var(--tx);
    text-align: center;
  }
  .dm-popup .dm-popup-sub {
    font-size: 12px;                       /* text-xs */
    margin-top: 4px;                       /* spacing-1 */
    color: var(--tx-2);
  }
  .dm-popup .dm-popup-divider {
    height: 1px;
    background: var(--ui-2);
    margin: 12px 0;                        /* spacing-3 */
  }
  .dm-popup .dm-popup-row {
    display: flex;
    gap: 16px;                             /* spacing-4 */
    align-items: baseline;
    font-family: var(--font-mono);
  }
  .dm-popup .dm-popup-row + .dm-popup-row {
    margin-top: 4px;                       /* spacing-1 */
  }
  .dm-popup .dm-popup-label {
    font-size: 12px;                       /* text-xs */
    color: var(--tx-2);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    min-width: 40px;                       /* spacing-10 */
  }
  .dm-popup .dm-popup-value {
    font-size: 14px;                       /* text-sm */
    color: var(--tx);
    font-variant-numeric: tabular-nums;
    margin-left: auto;
  }

  /* Shadow DOM 内では design-system.css の .resize-handle が届かないため、
     resize-handle.html の共通仕様を本要素内へ写す。変更時は同仕様と同期する。 */
  .resize-handle {
    position: absolute;
    left: 50%;
    width: var(--density-handle-width, 24px);
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

  class DensityMatrixDisplay extends HTMLElement {
    static get observedAttributes() {
      return ['span', 'state', 'placeholder', 'icon', 'no-popup', 'handle-state', 'visual-state'];
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
      const span = this.#span();
      const opts = {
        icon: this.hasAttribute('icon'),
        placeholder: this.hasAttribute('placeholder'),
        noPopup: this.hasAttribute('no-popup'),
        handleState: this.getAttribute('handle-state'),
        stateName: this.getAttribute('state') || (this.hasAttribute('icon') ? 'icon' : 'plus1'),
      };
      const info = this.#buildCanvas(span, opts);
      root.style.setProperty('--density-handle-width', `${Math.max(24, info.sizePx * 0.6)}px`);
      root.appendChild(info.canvas);
      if (!opts.icon) this.#attachResizeHandles(root, opts.handleState);
      if (!opts.icon && !opts.noPopup && !info.placeholder) this.#attachPopup(root, info);
    }

    #span() {
      const raw = Number(this.getAttribute('span') || '1');
      if (!Number.isFinite(raw)) return 1;
      return Math.min(8, Math.max(1, Math.trunc(raw)));
    }

    #buildCanvas(span, opts) {
      const n = 1 << span;
      const sizePx = 56 * span - 16;
      const cellPx = sizePx / n;
      const dpr = window.devicePixelRatio || 1;
      const canvas = document.createElement('canvas');
      canvas.className = 'dm-canvas';
      canvas.width = Math.round(sizePx * dpr);
      canvas.height = Math.round(sizePx * dpr);
      canvas.style.width = `${sizePx}px`;
      canvas.style.height = `${sizePx}px`;
      const ctx = canvas.getContext('2d');
      ctx.scale(dpr, dpr);

      ctx.fillStyle = opts.placeholder ? CANVAS_COLORS.purple100 : CANVAS_COLORS.bg;
      ctx.fillRect(0, 0, sizePx, sizePx);

      if (opts.placeholder) {
        ctx.strokeStyle = CANVAS_COLORS.ui2;
        ctx.lineWidth = 1;
        ctx.strokeRect(0.5, 0.5, sizePx - 1, sizePx - 1);
        return { canvas, rho: null, n, cellPx, span, sizePx, placeholder: true };
      }

      const rho = this.#buildRho(opts.stateName, n);
      const stroke = opts.icon ? 1.25 : cellPx > 24 ? 2 : 1;
      const halfStroke = stroke / 2;
      const outlineClearance = 1.5;
      const outlineRadius = opts.icon
        ? 7.875
        : Math.max(0, cellPx / 2 - halfStroke - outlineClearance);
      const innerRadius = opts.icon ? outlineRadius : Math.max(0, outlineRadius - halfStroke);
      const showFull = cellPx >= 12;
      const showOutline = cellPx >= 3;
      const useHeatmap = cellPx < 3;

      if (useHeatmap) {
        this.#drawHeatmap(ctx, rho, n, cellPx);
        ctx.strokeStyle = CANVAS_COLORS.ui2;
        ctx.lineWidth = 1;
        ctx.strokeRect(0.5, 0.5, sizePx - 1, sizePx - 1);
        return { canvas, rho, n, cellPx, span, sizePx, placeholder: false };
      }

      if (showOutline) {
        ctx.lineWidth = stroke;
        for (let r = 0; r < n; r++) {
          for (let c = 0; c < n; c++) {
            const k = (r * n + c) * 2;
            const re = rho[k];
            const im = rho[k + 1];
            const p = re * re + im * im;
            ctx.strokeStyle = p > 0.001 ? CANVAS_COLORS.tx2 : CANVAS_COLORS.ui2;
            ctx.beginPath();
            ctx.arc(c * cellPx + cellPx / 2, r * cellPx + cellPx / 2, outlineRadius, 0, Math.PI * 2);
            ctx.stroke();
          }
        }
      }

      const baseRadius = showOutline ? innerRadius : Math.max(0.5, cellPx / 2 - 0.5);
      this.#drawMagnitudeDisks(ctx, rho, n, cellPx, baseRadius);
      if (showFull) this.#drawNeedles(ctx, rho, n, cellPx, innerRadius, stroke);

      if (!opts.icon) {
        ctx.strokeStyle = CANVAS_COLORS.ui2;
        ctx.lineWidth = 1;
        ctx.strokeRect(0.5, 0.5, sizePx - 1, sizePx - 1);
      }

      return { canvas, rho, n, cellPx, span, sizePx, placeholder: false };
    }

    #drawHeatmap(ctx, rho, n, cellPx) {
      for (let r = 0; r < n; r++) {
        for (let c = 0; c < n; c++) {
          const k = (r * n + c) * 2;
          const re = rho[k];
          const im = rho[k + 1];
          const mag = Math.sqrt(re * re + im * im);
          if (mag < 1e-6) continue;
          const alpha = Math.sqrt(Math.min(1, mag));
          ctx.fillStyle = `rgba(146, 191, 219, ${alpha})`; // Flexoki blue-200
          ctx.fillRect(c * cellPx, r * cellPx, Math.ceil(cellPx) + 0.5, Math.ceil(cellPx) + 0.5);
        }
      }
    }

    #drawMagnitudeDisks(ctx, rho, n, cellPx, baseRadius) {
      for (let r = 0; r < n; r++) {
        for (let c = 0; c < n; c++) {
          const k = (r * n + c) * 2;
          const re = rho[k];
          const im = rho[k + 1];
          const mag = Math.sqrt(re * re + im * im);
          const radius = mag * baseRadius;
          if (radius < 0.3) continue;
          const cx = c * cellPx + cellPx / 2;
          const cy = r * cellPx + cellPx / 2;
          ctx.fillStyle = CANVAS_COLORS.blue200;
          ctx.beginPath();
          ctx.arc(cx, cy, radius, 0, Math.PI * 2);
          ctx.fill();
          if (radius >= 1.5) {
            ctx.strokeStyle = CANVAS_COLORS.blue400;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.arc(cx, cy, radius - 0.5, 0, Math.PI * 2);
            ctx.stroke();
          }
        }
      }
    }

    #drawNeedles(ctx, rho, n, cellPx, innerRadius, stroke) {
      ctx.strokeStyle = CANVAS_COLORS.tx;
      ctx.lineWidth = stroke;
      ctx.beginPath();
      for (let r = 0; r < n; r++) {
        for (let c = 0; c < n; c++) {
          if (r === c) continue;
          const k = (r * n + c) * 2;
          const re = rho[k];
          const im = rho[k + 1];
          const mag = Math.sqrt(re * re + im * im);
          if (mag < 0.001) continue;
          const cx = c * cellPx + cellPx / 2;
          const cy = r * cellPx + cellPx / 2;
          const ux = -im / mag;
          const uy = -re / mag;
          ctx.moveTo(cx, cy);
          ctx.lineTo(cx + ux * innerRadius, cy + uy * innerRadius);
        }
      }
      ctx.stroke();
    }

    #buildRho(name, n) {
      const buf = new Float32Array(n * n * 2);
      const set = (r, c, re, im) => {
        buf[(r * n + c) * 2] = re;
        buf[(r * n + c) * 2 + 1] = im;
      };
      if (name === 'plus1' || name === 'icon') {
        set(0, 0, 0.5, 0); set(0, 1, 0.5, 0);
        set(1, 0, 0.5, 0); set(1, 1, 0.5, 0);
      } else if (name === 'bell2') {
        set(0, 0, 0.5, 0); set(0, 3, 0.5, 0);
        set(3, 0, 0.5, 0); set(3, 3, 0.5, 0);
      } else if (name === 'mixed50') {
        this.#fillBellMix(buf, n, 0.5);
      } else if (name === 'maxmix2') {
        for (let k = 0; k < 4; k++) set(k, k, 0.25, 0);
      } else if (name === 'ghz3') {
        set(0, 0, 0.5, 0); set(0, 7, 0.5, 0);
        set(7, 0, 0.5, 0); set(7, 7, 0.5, 0);
      } else if (name === 'demo4') {
        this.#fillPureGaussian(buf, n, 0, 0.4, Math.PI / 4, 1, 0);
      } else if (name === 'demo') {
        this.#fillMixedDemo(buf, n);
      }
      return buf;
    }

    #fillBellMix(buf, n, pureWeight) {
      if (n !== 4) return;
      const mixWeight = 1 - pureWeight;
      const add = (r, c, re, im) => {
        const k = (r * n + c) * 2;
        buf[k] += re;
        buf[k + 1] += im;
      };
      for (let k = 0; k < 4; k++) add(k, k, mixWeight / 4, 0);
      add(0, 0, pureWeight * 0.5, 0);
      add(0, 3, pureWeight * 0.5, 0);
      add(3, 0, pureWeight * 0.5, 0);
      add(3, 3, pureWeight * 0.5, 0);
    }

    #fillPureGaussian(buf, n, center, decay, phaseStep, pure, mix) {
      const psi = new Float32Array(n * 2);
      let unity = 0;
      for (let k = 0; k < n; k++) {
        const mag = Math.exp(-Math.abs(k - center) * decay);
        const phase = k * phaseStep;
        const re = mag * Math.cos(phase);
        const im = mag * Math.sin(phase);
        psi[k * 2] = re;
        psi[k * 2 + 1] = im;
        unity += re * re + im * im;
      }
      const scale = unity > 0 ? 1 / Math.sqrt(unity) : 1;
      for (let k = 0; k < n * 2; k++) psi[k] *= scale;
      for (let r = 0; r < n; r++) {
        for (let c = 0; c < n; c++) {
          const a = psi[r * 2];
          const b = psi[r * 2 + 1];
          const x = psi[c * 2];
          const y = psi[c * 2 + 1];
          const k = (r * n + c) * 2;
          buf[k] = pure * (a * x + b * y) + (r === c ? mix / n : 0);
          buf[k + 1] = pure * (b * x - a * y);
        }
      }
    }

    #fillMixedDemo(buf, n) {
      const psi = new Float32Array(n * 2);
      const center = n / 4;
      const sigma = Math.max(2, n / 8);
      let unity = 0;
      for (let k = 0; k < n; k++) {
        const d = (k - center) / sigma;
        const mag = Math.exp(-(d * d) / 2);
        const phase = (2 * Math.PI * k) / Math.min(n, 32);
        const re = mag * Math.cos(phase);
        const im = mag * Math.sin(phase);
        psi[k * 2] = re;
        psi[k * 2 + 1] = im;
        unity += re * re + im * im;
      }
      if (unity > 0) {
        const scale = 1 / Math.sqrt(unity);
        for (let k = 0; k < n * 2; k++) psi[k] *= scale;
      }
      const pure = 0.85;
      const mix = 0.15;
      for (let r = 0; r < n; r++) {
        for (let c = 0; c < n; c++) {
          const a = psi[r * 2];
          const b = psi[r * 2 + 1];
          const x = psi[c * 2];
          const y = psi[c * 2 + 1];
          const k = (r * n + c) * 2;
          buf[k] = pure * (a * x + b * y) + (r === c ? mix / n : 0);
          buf[k + 1] = pure * (b * x - a * y);
        }
      }
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

    #attachPopup(root, info) {
      const popup = document.createElement('div');
      popup.className = 'dm-popup';
      root.appendChild(popup);
      const { canvas, rho, n, cellPx, span, sizePx } = info;
      canvas.addEventListener('mousemove', (event) => {
        const rect = canvas.getBoundingClientRect();
        const col = Math.floor((event.clientX - rect.left) / cellPx);
        const row = Math.floor((event.clientY - rect.top) / cellPx);
        if (col < 0 || col >= n || row < 0 || row >= n) {
          popup.style.display = 'none';
          return;
        }
        popup.innerHTML = this.#popupHtml(row, col, rho, span);
        popup.style.left = `${sizePx + 8}px`;
        popup.style.top = `${row * cellPx}px`;
        popup.style.display = 'block';
      });
      canvas.addEventListener('mouseleave', () => {
        popup.style.display = 'none';
      });
    }

    #popupHtml(row, col, rho, span) {
      const n = 1 << span;
      const k = (row * n + col) * 2;
      const re = rho[k];
      const im = rho[k + 1];
      if (row === col) {
        const p = re * 100;
        const db = p > 0 ? `${(Math.log10(p / 100) * 10).toFixed(1)} dB` : '−∞ dB';
        return `
          <div class="dm-popup-ket">Probability of |${this.#bin(row, span)}⟩</div>
          <div class="dm-popup-sub">diagonal · decimal ${row}</div>
          <div class="dm-popup-divider"></div>
          <div class="dm-popup-row"><span class="dm-popup-label">P</span><span class="dm-popup-value">${p.toFixed(4)}%</span></div>
          <div class="dm-popup-row"><span class="dm-popup-label">log</span><span class="dm-popup-value">${db.replace(/^-/, '−')}</span></div>
        `;
      }
      const mag = Math.sqrt(re * re + im * im);
      const phase = Math.atan2(im, re) * 180 / Math.PI;
      const phaseSign = phase >= 0 ? '+' : '−';
      const imSign = im >= 0 ? '+' : '−';
      return `
        <div class="dm-popup-ket">Coupling of |${this.#bin(row, span)}⟩ to ⟨${this.#bin(col, span)}|</div>
        <div class="dm-popup-sub">off-diagonal · decimal ${row} to ${col}</div>
        <div class="dm-popup-divider"></div>
        <div class="dm-popup-row"><span class="dm-popup-label">val</span><span class="dm-popup-value">${re.toFixed(6)} ${imSign} ${Math.abs(im).toFixed(6)}i</span></div>
        <div class="dm-popup-row"><span class="dm-popup-label">mag</span><span class="dm-popup-value">${mag.toFixed(6)}</span></div>
        <div class="dm-popup-row"><span class="dm-popup-label">phase</span><span class="dm-popup-value">${phaseSign}${Math.abs(phase).toFixed(2)}°</span></div>
      `;
    }

    #bin(k, span) {
      return k.toString(2).padStart(span, '0');
    }
  }

  customElements.define(ELEMENT_NAME, DensityMatrixDisplay);
})();
