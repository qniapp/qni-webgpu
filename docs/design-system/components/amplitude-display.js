/* ─────────────────────────────────────────────────────────────────────
 * amplitude-display.js — <amplitude-display> カスタム要素
 *
 * 振幅表示ブロックのデザイン仕様モックを 1 箇所へ集約する。span / sample /
 * placeholder / incoherent / icon / no-popup / handle-state 属性で仕様ページの
 * 各状態を表す。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const ELEMENT_NAME = 'amplitude-display';
  if (customElements.get(ELEMENT_NAME)) return;

  const BG = '#FFFCF0';         // Flexoki bg
  const BLUE_200 = '#92BFDB';   // Flexoki blue-200: amplitude disk fill
  const BLUE_400 = '#4385BE';   // Flexoki blue-400: disk border
  const UI_2 = '#DAD8CE';       // Flexoki ui-2: zero outline / frame
  const TX_2 = '#6F6E69';       // Flexoki tx-2: non-zero outline
  const TX = '#100F0F';         // Flexoki tx: phase needle
  const PURPLE_100 = '#ECE1F3'; // Flexoki purple-100: placeholder fill

  const STYLE = `
<style>
  :host {
    display: inline-flex;
    position: relative;
    flex-direction: column;
    align-items: center;
    flex-shrink: 0;
  }
  .canvas-shell {
    display: inline-flex;
    position: relative;
    align-items: stretch;
    flex-shrink: 0;
  }
  canvas {
    display: block;
    cursor: crosshair;
  }
  :host([icon]) canvas {
    cursor: default;
  }
  :host([state="hover"]) canvas {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--purple-400);
  }
  .cell-hover {
    position: absolute;
    display: none;
    border: 2px solid var(--purple-400); /* Flexoki purple-400 */
    border-radius: 50%;
    box-sizing: border-box;
    pointer-events: none;
    z-index: 12;
  }

  .ad-popup {
    position: absolute;
    display: none;
    pointer-events: none;
    z-index: 50;
    background: var(--bg);
    border: 1px solid var(--tx-3);
    border-radius: 10px;
    padding: 12px 16px;
    font-family: var(--font-body);
    font-size: 12px;        /* text-xs */
    line-height: 16px;
    color: var(--tx);
    white-space: nowrap;
    box-shadow: 0 10px 28px rgba(16, 15, 15, 0.14);
  }
  .ad-popup .ad-popup-ket {
    font-family: var(--font-mono);
    font-size: 14px;        /* text-sm */
    font-weight: 400;
    letter-spacing: 0.02em;
    color: var(--tx);
    text-align: center;
  }
  .ad-popup .ad-popup-sub {
    font-size: 12px;        /* text-xs */
    margin-top: 4px;        /* spacing-1 */
    color: var(--tx-2);
  }
  .ad-popup .ad-popup-divider {
    height: 1px;
    background: var(--ui-2);
    margin: 12px 0;         /* spacing-3 */
  }
  .ad-popup .ad-popup-row {
    display: flex;
    gap: 16px;              /* spacing-4 */
    align-items: baseline;
    font-family: var(--font-mono);
  }
  .ad-popup .ad-popup-row + .ad-popup-row { margin-top: 4px; }
  .ad-popup .ad-popup-label {
    font-size: 12px;        /* text-xs */
    color: var(--tx-2);
    text-transform: uppercase;
    letter-spacing: 0.12em;
    min-width: 40px;
  }
  .ad-popup .ad-popup-value {
    font-size: 14px;        /* text-sm */
    color: var(--tx);
    font-variant-numeric: tabular-nums;
    margin-left: auto;
  }
  .ad-popup .doc-popover-tail {
    position: absolute;
    top: var(--tail-y, 50%);
    width: 10px;
    height: 16px;
    transform: translateY(-50%);
    pointer-events: none;
    overflow: visible;
  }
  .ad-popup .doc-popover-tail__fill { fill: var(--bg); }
  .ad-popup .doc-popover-tail__stroke {
    fill: none;
    stroke: var(--tx-3);
    stroke-width: 1.5px;
    stroke-linejoin: round;
  }
  .ad-popup .doc-popover-tail--right { left: -8px; }
  .ad-popup .doc-popover-tail--left { right: -8px; display: none; }
  .ad-popup[data-side="left"] .doc-popover-tail--right { display: none; }
  .ad-popup[data-side="left"] .doc-popover-tail--left { display: block; }

  .incoherent-label {
    margin-top: 16px;       /* spacing-4: 下リサイズハンドルと重ねない */
    font-family: var(--font-mono);
    font-size: 12px;        /* text-xs */
    line-height: 16px;
    color: var(--red-600);  /* Flexoki red-600 */
  }

  /* Shadow DOM 内では design-system.css の .resize-handle が届かないため、
     resize-handle.html の共通仕様を本要素内へ写す。変更時は同仕様と同期する。 */
  .resize-handle {
    position: absolute;
    left: 50%;
    height: 6px;
    background: var(--cyan-400);  /* Flexoki cyan-400 */
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
    background: var(--cyan-600);  /* Flexoki cyan-600 */
    transform: translateX(-50%) scale(1.15);
    transition: none;
  }
  .resize-handle:active,
  .resize-handle[data-handle-state="active"] {
    background: var(--cyan-600);  /* Flexoki cyan-600 */
    transform: translateX(-50%) scale(1.25);
    transition: none;
  }
</style>
`;

  const clamp = (value, min, max) => Math.max(min, Math.min(max, value));

  function numberAttr(el, name, fallback) {
    const raw = el.getAttribute(name);
    if (raw === null || raw.trim() === '') return fallback;
    const value = Number(raw);
    return Number.isFinite(value) ? value : fallback;
  }

  class AmplitudeDisplay extends HTMLElement {
    static get observedAttributes() {
      return ['cell', 'handle-state', 'icon', 'incoherent', 'no-popup', 'placeholder', 'sample', 'span', 'state'];
    }

    constructor() {
      super();
      this.attachShadow({ mode: 'open' });
    }

    connectedCallback() {
      this.#render();
    }

    attributeChangedCallback() {
      if (this.isConnected) this.#render();
    }

    #render() {
      const shadow = this.shadowRoot;
      shadow.innerHTML = STYLE;
      const info = this.#buildCanvas();
      const shell = document.createElement('span');
      shell.className = 'canvas-shell';
      shell.appendChild(info.canvas);
      shadow.appendChild(shell);
      if (!this.hasAttribute('icon')) this.#attachResizeHandles(shell, info.cssWidth);
      if (!this.hasAttribute('icon') && !this.hasAttribute('placeholder')) this.#attachCellHover(shell, info);
      if (!this.hasAttribute('icon') && !this.hasAttribute('no-popup') && !this.hasAttribute('placeholder')) {
        this.#attachPopup(shell, info);
      }
      if (info.incoherent && !this.hasAttribute('icon') && !this.hasAttribute('placeholder')) {
        const label = document.createElement('span');
        label.className = 'incoherent-label';
        label.textContent = 'incoherent';
        shadow.appendChild(label);
      }
    }

    #buildCanvas() {
      const layout = this.#layout();
      const { wGrid, hGrid, n, cols, span, cellPx, sample, incoherent, icon, placeholder } = layout;
      const cssWidth = cellPx * wGrid;
      const cssHeight = cellPx * hGrid;
      const dpr = window.devicePixelRatio || 1;
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(cssWidth * dpr);
      canvas.height = Math.round(cssHeight * dpr);
      canvas.style.width = `${cssWidth}px`;
      canvas.style.height = `${cssHeight}px`;
      canvas.setAttribute('role', 'img');
      canvas.setAttribute('aria-label', `振幅表示ブロック span ${span}`);
      const ctx = canvas.getContext('2d');
      ctx.scale(dpr, dpr);

      const rawState = this.#buildState(sample, n);
      const lockIndex = (incoherent || icon || placeholder) ? null : this.#pickPhaseLockIndex(rawState, n);
      const state = lockIndex === null ? rawState : this.#applyPhaseLock(rawState, n, lockIndex);

      ctx.fillStyle = placeholder ? PURPLE_100 : BG;
      ctx.fillRect(0, 0, cssWidth, cssHeight);

      if (placeholder) {
        this.#drawPlaceholder(ctx, wGrid, hGrid, cellPx, cssWidth, cssHeight);
        return { canvas, cssWidth, cssHeight, state, wGrid, hGrid, cellPx, lockIndex: null, incoherent: false, span };
      }

      const cellsForEach = (fn) => {
        for (let row = 0; row < hGrid; row++) {
          for (let col = 0; col < wGrid; col++) {
            const k = row * wGrid + col;
            const re = state[k * 2];
            const im = state[k * 2 + 1];
            fn(re, im, col * cellPx, row * cellPx, k);
          }
        }
      };

      const stroke = cellPx >= 24 ? 2 : 1;
      const halfStroke = stroke / 2;
      const outlineRadius = Math.max(0, cellPx / 2 - halfStroke - 1.5);
      const innerRadius = Math.max(0, outlineRadius - halfStroke);
      const showOutline = cellPx >= 3;
      const showNeedle = cellPx >= 12 && !incoherent;
      const useHeatmap = cellPx < 3;

      if (!icon) {
        ctx.strokeStyle = UI_2;
        ctx.lineWidth = 1;
        ctx.strokeRect(0.5, 0.5, Math.max(0, cssWidth - 1), Math.max(0, cssHeight - 1));
      }

      if (showOutline) {
        ctx.lineWidth = stroke;
        cellsForEach((re, im, x, y) => {
          const mag = Math.sqrt(re * re + im * im);
          ctx.strokeStyle = mag > 0.000001 ? TX_2 : UI_2;
          ctx.beginPath();
          ctx.arc(x + cellPx / 2, y + cellPx / 2, outlineRadius, 0, Math.PI * 2);
          ctx.stroke();
        });
      }

      if (useHeatmap) {
        this.#drawHeatmap(ctx, state, n, cellsForEach, incoherent, cellPx);
      } else if (!incoherent) {
        this.#drawCoherentDisks(ctx, cellsForEach, showOutline, innerRadius, cellPx);
      } else {
        this.#drawIncoherentDisks(ctx, cellsForEach, showOutline, innerRadius, cellPx);
      }

      if (showNeedle) this.#drawNeedles(ctx, cellsForEach, innerRadius, cellPx, stroke);

      return { canvas, cssWidth, cssHeight, state, wGrid, hGrid, cellPx, lockIndex, incoherent, span };
    }

    #layout() {
      const icon = this.hasAttribute('icon');
      if (icon) {
        return {
          wGrid: 1,
          hGrid: 1,
          n: 1,
          cols: 1,
          span: 1,
          cellPx: clamp(numberAttr(this, 'cell', 40), 1, 256),
          sample: this.getAttribute('sample') || 'icon',
          incoherent: false,
          icon: true,
          placeholder: false,
        };
      }
      const span = clamp(Math.trunc(numberAttr(this, 'span', 1)), 1, 16);
      const g = this.#gridForSpan(span);
      const sample = this.getAttribute('sample') || (this.hasAttribute('placeholder') ? 'placeholder' : this.#defaultSample(span));
      const cellPx = this.#cellPxForGrid(g.w, g.h, g.cols, span, this.getAttribute('cell'));
      return {
        wGrid: g.w,
        hGrid: g.h,
        n: g.n,
        cols: g.cols,
        span,
        cellPx,
        sample,
        incoherent: this.hasAttribute('incoherent'),
        icon: false,
        placeholder: this.hasAttribute('placeholder'),
      };
    }

    #defaultSample(span) {
      if (span === 1) return 'hadamard1';
      if (span === 2) return 'mixed2';
      if (span === 3) return 'qft3';
      if (span === 4) return 'peak4';
      return 'demo';
    }

    #drawPlaceholder(ctx, wGrid, hGrid, cellPx, cssWidth, cssHeight) {
      const stroke = cellPx >= 24 ? 2 : 1;
      const halfStroke = stroke / 2;
      const r = Math.max(0, cellPx / 2 - halfStroke - 1.5);
      ctx.strokeStyle = UI_2;
      ctx.lineWidth = stroke;
      for (let row = 0; row < hGrid; row++) {
        for (let col = 0; col < wGrid; col++) {
          ctx.beginPath();
          ctx.arc(col * cellPx + cellPx / 2, row * cellPx + cellPx / 2, r, 0, Math.PI * 2);
          ctx.stroke();
        }
      }
      ctx.lineWidth = 1;
      ctx.strokeRect(0.5, 0.5, Math.max(0, cssWidth - 1), Math.max(0, cssHeight - 1));
    }

    #drawHeatmap(ctx, state, n, cellsForEach, incoherent, cellPx) {
      let maxMag = 0;
      for (let k = 0; k < n; k++) {
        const re = state[k * 2];
        const im = state[k * 2 + 1];
        const mag = Math.sqrt(re * re + im * im);
        if (mag > maxMag) maxMag = mag;
      }
      const denom = maxMag > 1e-12 ? maxMag : 1;
      cellsForEach((re, im, x, y) => {
        const mag = Math.sqrt(re * re + im * im);
        if (mag < 1e-6) return;
        const alpha = (incoherent ? 0.45 : 1) * Math.sqrt(mag / denom);
        ctx.fillStyle = `rgba(146, 191, 219, ${alpha})`;
        ctx.fillRect(x, y, cellPx, cellPx);
      });
    }

    #drawCoherentDisks(ctx, cellsForEach, showOutline, innerRadius, cellPx) {
      const rBase = showOutline ? innerRadius : Math.max(0.5, cellPx / 2 - 0.5);
      cellsForEach((re, im, x, y) => {
        const mag = Math.sqrt(re * re + im * im);
        const r = mag * rBase;
        if (r < 0.3) return;
        const cx = x + cellPx / 2;
        const cy = y + cellPx / 2;
        ctx.fillStyle = BLUE_200;
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.fill();
        if (r >= 1.5) {
          ctx.strokeStyle = BLUE_400;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.arc(cx, cy, r - 0.5, 0, Math.PI * 2);
          ctx.stroke();
        }
      });
    }

    #drawIncoherentDisks(ctx, cellsForEach, showOutline, innerRadius, cellPx) {
      const rBase = showOutline ? innerRadius : Math.max(0.5, cellPx / 2 - 0.5);
      cellsForEach((re, im, x, y) => {
        const sqrtProb = Math.sqrt(re * re + im * im);
        const r = sqrtProb * rBase;
        if (r < 0.3) return;
        const cx = x + cellPx / 2;
        const cy = y + cellPx / 2;
        ctx.globalAlpha = 0.45;
        ctx.fillStyle = BLUE_200;
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.fill();
        if (r >= 1.5) {
          ctx.strokeStyle = BLUE_400;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.arc(cx, cy, r - 0.5, 0, Math.PI * 2);
          ctx.stroke();
        }
        ctx.globalAlpha = 1;
      });
    }

    #drawNeedles(ctx, cellsForEach, innerRadius, cellPx, stroke) {
      ctx.strokeStyle = TX;
      ctx.lineWidth = stroke;
      ctx.beginPath();
      cellsForEach((re, im, x, y) => {
        const mag = Math.sqrt(re * re + im * im);
        if (mag < 0.001) return;
        const cx = x + cellPx / 2;
        const cy = y + cellPx / 2;
        const ux = -im / mag;
        const uy = -re / mag;
        ctx.moveTo(cx, cy);
        ctx.lineTo(cx + ux * innerRadius, cy + uy * innerRadius);
      });
      ctx.stroke();
    }

    #attachCellHover(parent, info) {
      const { canvas, wGrid, hGrid, cellPx } = info;
      const stroke = cellPx >= 24 ? 2 : 1;
      const outlineRadius = Math.max(0, cellPx / 2 - stroke / 2 - 1.5);
      if (outlineRadius <= 0) return;
      const diameter = outlineRadius * 2 + stroke;
      const hover = document.createElement('div');
      hover.className = 'cell-hover';
      hover.style.borderWidth = `${stroke}px`;
      hover.style.width = `${diameter}px`;
      hover.style.height = `${diameter}px`;
      parent.appendChild(hover);
      canvas.addEventListener('mousemove', (event) => {
        const rect = canvas.getBoundingClientRect();
        const col = Math.floor((event.clientX - rect.left) / cellPx);
        const row = Math.floor((event.clientY - rect.top) / cellPx);
        if (col < 0 || col >= wGrid || row < 0 || row >= hGrid) {
          hover.style.display = 'none';
          return;
        }
        hover.style.left = `${col * cellPx + cellPx / 2 - diameter / 2}px`;
        hover.style.top = `${row * cellPx + cellPx / 2 - diameter / 2}px`;
        hover.style.display = 'block';
      });
      canvas.addEventListener('mouseleave', () => {
        hover.style.display = 'none';
      });
    }

    #attachResizeHandles(parent, cssWidth) {
      const width = Math.max(24, Math.round(cssWidth * 0.6));
      const handleState = this.getAttribute('handle-state');
      const top = this.#createResizeHandle('top', width, handleState);
      const bottom = this.#createResizeHandle('bottom', width, handleState ? 'visible' : undefined);
      parent.appendChild(top);
      parent.appendChild(bottom);
    }

    #createResizeHandle(position, width, state) {
      const el = document.createElement('div');
      el.className = `resize-handle resize-handle--${position}`;
      el.style.width = `${width}px`;
      el.setAttribute('aria-hidden', 'true');
      if (state) el.dataset.handleState = state;
      return el;
    }

    #attachPopup(parent, info) {
      const popup = document.createElement('div');
      popup.className = 'ad-popup';
      parent.appendChild(popup);
      const { canvas, state, wGrid, hGrid, cellPx, incoherent } = info;
      const span = Math.log2(wGrid * hGrid);
      canvas.addEventListener('mousemove', (event) => {
        const rect = canvas.getBoundingClientRect();
        const col = Math.floor((event.clientX - rect.left) / cellPx);
        const row = Math.floor((event.clientY - rect.top) / cellPx);
        if (col < 0 || col >= wGrid || row < 0 || row >= hGrid) {
          popup.style.display = 'none';
          return;
        }
        const idx = row * wGrid + col;
        const re = state[idx * 2];
        const im = state[idx * 2 + 1];
        popup.innerHTML = this.#popupHtml(idx, re, im, span, incoherent) + this.#popupTailHtml();
        this.#placePopup(parent, popup, {
          left: canvas.offsetLeft + col * cellPx,
          width: cellPx,
        }, canvas.offsetTop + row * cellPx + cellPx / 2);
      });
      canvas.addEventListener('mouseleave', () => {
        popup.style.display = 'none';
      });
    }

    #placePopup(anchor, popup, targetRect, cellCenterY) {
      const hostRect = anchor.getBoundingClientRect();
      popup.style.display = 'block';
      popup.style.visibility = 'hidden';
      const sideGap = 12;
      const pad = 4;
      const width = popup.offsetWidth;
      const height = popup.offsetHeight;
      const rightLeft = targetRect.left + targetRect.width + sideGap;
      const leftLeft = targetRect.left - sideGap - width;
      const preferRight = hostRect.left + rightLeft + width <= window.innerWidth - pad;
      const minLeft = pad - hostRect.left;
      const maxLeft = window.innerWidth - pad - hostRect.left - width;
      const minTop = pad - hostRect.top;
      const maxTop = window.innerHeight - pad - hostRect.top - height;
      const unclampedLeft = preferRight ? rightLeft : leftLeft;
      const left = Math.max(minLeft, Math.min(unclampedLeft, Math.max(minLeft, maxLeft)));
      const top = Math.max(minTop, Math.min(cellCenterY - height / 2, Math.max(minTop, maxTop)));
      const side = left + width / 2 >= targetRect.left + targetRect.width / 2 ? 'right' : 'left';
      const rawTailY = cellCenterY - top;
      const tailY = Math.max(8, Math.min(rawTailY, Math.max(8, height - 8)));
      popup.dataset.side = side;
      popup.style.setProperty('--tail-y', `${tailY}px`);
      popup.style.left = `${left}px`;
      popup.style.top = `${top}px`;
      popup.style.visibility = 'visible';
    }

    #buildState(name, n) {
      const buf = new Float32Array(n * 2);
      if (name === 'hadamard1') {
        const v = 1 / Math.sqrt(2);
        buf[0] = v; buf[1] = 0;
        buf[2] = v; buf[3] = 0;
      } else if (name === 'mixed2') {
        buf[0] = 0.4; buf[1] = 0.0;
        buf[2] = 0.5; buf[3] = 0.3;
        buf[4] = 0.3; buf[5] = -0.4;
        buf[6] = 0.5; buf[7] = 0.0;
      } else if (name === 'qft3') {
        const a = 1 / Math.sqrt(8);
        for (let k = 0; k < 8; k++) {
          const phase = (k * 2 * Math.PI) / 8;
          buf[k * 2] = a * Math.cos(phase);
          buf[k * 2 + 1] = a * Math.sin(phase);
        }
      } else if (name === 'peak4') {
        let unity = 0;
        for (let k = 0; k < 16; k++) {
          const decay = Math.exp(-k * 0.35);
          const phase = k * 0.4;
          const re = decay * Math.cos(phase);
          const im = decay * Math.sin(phase);
          buf[k * 2] = re;
          buf[k * 2 + 1] = im;
          unity += re * re + im * im;
        }
        const u = 1 / Math.sqrt(unity);
        for (let k = 0; k < 32; k++) buf[k] *= u;
      } else if (name === 'incoherent2') {
        buf[0] = Math.sqrt(0.25); buf[1] = 0;
        buf[2] = Math.sqrt(0.30); buf[3] = 0;
        buf[4] = Math.sqrt(0.20); buf[5] = 0;
        buf[6] = Math.sqrt(0.25); buf[7] = 0;
      } else if (name === 'demo') {
        const center = n / 4;
        const sigma = Math.max(2, n / 8);
        let unity = 0;
        for (let k = 0; k < n; k++) {
          const d = (k - center) / sigma;
          const mag = Math.exp(-(d * d) / 2);
          const phase = (2 * Math.PI * k) / Math.min(n, 32);
          const re = mag * Math.cos(phase);
          const im = mag * Math.sin(phase);
          buf[k * 2] = re;
          buf[k * 2 + 1] = im;
          unity += re * re + im * im;
        }
        if (unity > 0) {
          const u = 1 / Math.sqrt(unity);
          for (let k = 0; k < n * 2; k++) buf[k] *= u;
        }
      } else if (name === 'icon') {
        buf[0] = -0.5;
        buf[1] = -0.5;
      }
      return buf;
    }

    #pickPhaseLockIndex(state, n) {
      let best = 0;
      let result = 0;
      for (let k = 0; k < n; k++) {
        const re = state[k * 2];
        const im = state[k * 2 + 1];
        const mag2 = re * re + im * im;
        if (mag2 > best * 10000) {
          best = mag2;
          result = k;
        }
      }
      return result;
    }

    #applyPhaseLock(state, n, lockIndex) {
      const out = new Float32Array(state.length);
      const lr = state[lockIndex * 2];
      const li = state[lockIndex * 2 + 1];
      const phase = Math.atan2(li, lr);
      const c = Math.cos(-phase);
      const s = Math.sin(-phase);
      for (let k = 0; k < n; k++) {
        const re = state[k * 2];
        const im = state[k * 2 + 1];
        out[k * 2] = re * c - im * s;
        out[k * 2 + 1] = re * s + im * c;
      }
      return out;
    }

    #gridForSpan(span) {
      const n = 1 << span;
      const w = n === 2 ? 2 : (1 << Math.floor(Math.log2(n) / 2));
      const cols = span === 1 ? 2 : (span % 2 === 0 ? span : Math.ceil(span / 2));
      return { n, w, h: n / w, cols };
    }

    #cellPxForGrid(wGrid, hGrid, cols, span, cellAttr) {
      const hint = cellAttr === null ? NaN : Number(cellAttr);
      if (Number.isFinite(hint)) return clamp(hint, 1, 256);
      const specW = 56 * cols - 16;
      const specH = 56 * span - 16;
      const cellSpec = Math.min(specW / wGrid, specH / hGrid);
      const scale = Math.min(1, 320 / Math.max(specW, specH));
      return Math.max(1, Math.floor(cellSpec * scale));
    }

    #bin(k, span) {
      let s = k.toString(2);
      while (s.length < span) s = `0${s}`;
      return s;
    }

    #popupHtml(idx, re, im, span, incoherent) {
      const ket = this.#bin(idx, span);
      const mag2 = (re * re + im * im) * 100;
      if (incoherent) {
        const db = mag2 > 0 ? `${(Math.log10(mag2 / 100) * 10).toFixed(1)} dB` : '−∞ dB';
        return `
          <div class="ad-popup-ket">|${ket}⟩</div>
          <div class="ad-popup-sub">Probability · decimal ${idx} · [entangled with other qubits]</div>
          <div class="ad-popup-divider"></div>
          <div class="ad-popup-row"><span class="ad-popup-label">raw</span><span class="ad-popup-value">${mag2.toFixed(4)}%</span></div>
          <div class="ad-popup-row"><span class="ad-popup-label">log</span><span class="ad-popup-value">${db.replace(/^-/, '−')}</span></div>
        `;
      }
      const phase = Math.atan2(im, re) * 180 / Math.PI;
      const phaseStr = `${phase >= 0 ? '+' : '−'}${Math.abs(phase).toFixed(2)}°`;
      const sign = im >= 0 ? '+' : '−';
      return `
        <div class="ad-popup-ket">|${ket}⟩</div>
        <div class="ad-popup-sub">Amplitude · decimal ${idx}</div>
        <div class="ad-popup-divider"></div>
        <div class="ad-popup-row"><span class="ad-popup-label">val</span><span class="ad-popup-value">${re.toFixed(5)} ${sign} ${Math.abs(im).toFixed(5)}i</span></div>
        <div class="ad-popup-row"><span class="ad-popup-label">mag²</span><span class="ad-popup-value">${mag2.toFixed(4)}%</span></div>
        <div class="ad-popup-row"><span class="ad-popup-label">phase</span><span class="ad-popup-value">${phaseStr}</span></div>
      `;
    }

    #popupTailHtml() {
      return `
        <svg class="doc-popover-tail doc-popover-tail--right" viewBox="0 0 10 16" aria-hidden="true">
          <path class="doc-popover-tail__fill" d="M10 0 L0 8 L10 16 Z"></path>
          <path class="doc-popover-tail__stroke" d="M8 0 L0 8 L8 16"></path>
        </svg>
        <svg class="doc-popover-tail doc-popover-tail--left" viewBox="0 0 10 16" aria-hidden="true">
          <path class="doc-popover-tail__fill" d="M0 0 L10 8 L0 16 Z"></path>
          <path class="doc-popover-tail__stroke" d="M2 0 L10 8 L2 16"></path>
        </svg>
      `;
    }
  }

  customElements.define(ELEMENT_NAME, AmplitudeDisplay);
})();
