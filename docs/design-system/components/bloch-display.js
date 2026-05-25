/* ─────────────────────────────────────────────────────────────────────
 * bloch-display.js — <bloch-display> カスタム要素
 *
 * ブロッホ球表示ブロックの正規描画を 1 箇所へ集約する。
 * x / y / z / icon / no-popover / state / zoom 属性で仕様ページの各状態を表す。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  const SVG_NS = 'http://www.w3.org/2000/svg';
  const EPS = 1e-3;
  const RADIUS = 19;
  const CENTER = 20;
  const TIP_RADIUS = 4;
  const COLORS = Object.freeze({
    red300: [232, 112, 95],    // Flexoki red-300 / brighter saturated trial / |0⟩
    purple400: [139, 126, 200], // Flexoki purple-400 / brighter saturated trial / equator
    blue300: [102, 160, 200],   // Flexoki blue-300 / brighter saturated trial / |1⟩
  });

  const TEMPLATE = document.createElement('template');
  TEMPLATE.innerHTML = `
<style>
  :host {
    display: inline-block;
    width: 40px;
    height: 40px;
    position: relative;
    overflow: visible;
    flex-shrink: 0;
    border-radius: 6px;
  }
  :host(:hover),
  :host([state="hover"]) {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--purple-400);
  }
  :host([icon]:hover),
  :host([icon][state="hover"]) {
    box-shadow: none;
  }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .sphere {
    fill: var(--bg-2);                 /* Flexoki bg-2 / bloch_sphere_bg */
    stroke: var(--tx-3);               /* Flexoki tx-3 / bloch_sphere_lines */
    stroke-width: 1.5;
  }
  .axis {
    stroke: var(--tx-3);               /* Flexoki tx-3 / bloch_sphere_lines */
    stroke-width: 1;
    fill: none;
  }
  .vec {
    stroke: var(--tx);                 /* Flexoki tx / bloch_vector_line */
    stroke-width: 1.5;
    stroke-linecap: round;
  }
  :host([zoom]) .axis { stroke-width: 0.25; }
  :host([zoom]) .vec { stroke-width: 0.375; }
  .tip {
    fill: var(--red-300);              /* Flexoki red-300 / brighter saturated trial */
    stroke: var(--tx);                 /* Flexoki tx / 1px black outline */
    stroke-width: 1;
  }
  .zero {
    fill: var(--tx-3);                 /* Flexoki tx-3 / inactive */
    stroke: var(--tx);                 /* Flexoki tx / 1px black outline */
    stroke-width: 1;
  }

  .bd-popover {
    position: absolute;
    top: 0;
    left: calc(100% + 12px);             /* tail 8 px + gap 4 px */
    display: none;
    pointer-events: none;
    z-index: 50;
    background: var(--bg);               /* Flexoki bg / popover_surface */
    border: 1px solid var(--tx-3);       /* Flexoki tx-3 / popover_outline */
    border-radius: 10px;
    padding: 12px 16px;                  /* spacing-3 / spacing-4 */
    font-family: 'Geist', sans-serif;
    font-size: 12px;                     /* text-xs */
    line-height: 16px;                   /* text-xs default */
    color: var(--tx);                    /* Flexoki tx */
    white-space: nowrap;
    box-shadow: 0 10px 28px rgba(16, 15, 15, 0.14); /* Flexoki tx alpha */
  }
  :host(:not([icon]):not([no-popover]):hover) .bd-popover,
  :host(:not([icon]):not([no-popover])[state="hover"]) .bd-popover {
    display: block;
  }
  .bd-popover-title {
    font-size: 14px;                     /* text-sm */
    line-height: 20px;                   /* text-sm default */
    font-weight: 400;
    color: var(--tx);                    /* Flexoki tx */
    letter-spacing: -0.005em;
  }
  .bd-popover-divider {
    height: 1px;
    background: var(--ui-2);             /* Flexoki ui-2 */
    margin: 12px 0;                      /* spacing-3 */
  }
  .bd-popover-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;                           /* spacing-3 */
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .bd-popover-row + .bd-popover-row { margin-top: 4px; }
  .bd-popover-cell {
    display: flex;
    align-items: baseline;
    gap: 8px;                            /* spacing-2 */
  }
  .bd-popover-key {
    font-size: 12px;                     /* text-xs */
    line-height: 16px;                   /* text-xs default */
    color: var(--tx-2);                  /* Flexoki tx-2 */
    min-width: 12px;
  }
  .bd-popover-val {
    font-size: 14px;                     /* text-sm */
    line-height: 20px;                   /* text-sm default */
    color: var(--tx);                    /* Flexoki tx */
  }
  .bd-popover-tail {
    position: absolute;
    top: 16px;                           /* spacing-4: tail center = 20px host center */
    left: -12px;                         /* tail 8 px + gap 4 px */
    width: 16px;
    height: 8px;
    transform: rotate(90deg);
    pointer-events: none;
    overflow: visible;
  }
  .bd-popover-tail-fill { fill: var(--bg); }
  .bd-popover-tail-stroke {
    fill: none;
    stroke: var(--tx-3);                 /* Flexoki tx-3 / popover_outline */
    stroke-width: 1.5px;
    stroke-linejoin: round;
  }
</style>
<div class="render-root"></div>
`;

  class BlochDisplay extends HTMLElement {
    static get observedAttributes() {
      return ['x', 'y', 'z', 'icon', 'no-popover', 'state', 'zoom'];
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
      const vector = this.#vector();
      const svg = this.#buildSvg(vector);
      root.appendChild(svg);
      if (!this.hasAttribute('icon') && !this.hasAttribute('no-popover')) {
        root.appendChild(this.#buildPopover(vector));
      }
    }

    #vector() {
      return {
        x: this.#numberAttr('x', 0),
        y: this.#numberAttr('y', 0),
        z: this.#numberAttr('z', 0),
      };
    }

    #numberAttr(name, fallback) {
      const value = Number(this.getAttribute(name));
      return Number.isFinite(value) ? value : fallback;
    }

    #buildSvg(vector) {
      const svg = document.createElementNS(SVG_NS, 'svg');
      svg.setAttribute('viewBox', '0 0 40 40');
      svg.setAttribute('xmlns', SVG_NS);
      svg.setAttribute('aria-hidden', 'true');
      svg.appendChild(this.#circle('sphere', CENTER, CENTER, RADIUS));
      svg.appendChild(this.#line('axis', 1, 20, 39, 20));
      svg.appendChild(this.#line('axis', 20, 1, 20, 39));
      svg.appendChild(this.#line('axis', 14.3, 25.7, 25.7, 14.3));
      svg.appendChild(this.#ellipse('axis', CENTER, CENTER, 6.84, 19));
      svg.appendChild(this.#ellipse('axis', CENTER, CENTER, 19, 6.84));

      const magnitude = this.#magnitude(vector);
      if (magnitude < EPS) {
        svg.appendChild(this.#circle('zero', CENTER, CENTER, TIP_RADIUS));
        return svg;
      }

      const tip = this.#project(vector);
      svg.appendChild(this.#line('vec', CENTER, CENTER, tip.x, tip.y));
      const dot = this.#circle('tip', tip.x, tip.y, TIP_RADIUS);
      dot.style.fill = this.#tipColor(vector.z);
      svg.appendChild(dot);
      return svg;
    }

    #line(className, x1, y1, x2, y2) {
      const line = document.createElementNS(SVG_NS, 'line');
      line.setAttribute('class', className);
      line.setAttribute('x1', this.#fmtCoord(x1));
      line.setAttribute('y1', this.#fmtCoord(y1));
      line.setAttribute('x2', this.#fmtCoord(x2));
      line.setAttribute('y2', this.#fmtCoord(y2));
      return line;
    }

    #circle(className, cx, cy, r) {
      const circle = document.createElementNS(SVG_NS, 'circle');
      circle.setAttribute('class', className);
      circle.setAttribute('cx', this.#fmtCoord(cx));
      circle.setAttribute('cy', this.#fmtCoord(cy));
      circle.setAttribute('r', this.#fmtCoord(r));
      return circle;
    }

    #ellipse(className, cx, cy, rx, ry) {
      const ellipse = document.createElementNS(SVG_NS, 'ellipse');
      ellipse.setAttribute('class', className);
      ellipse.setAttribute('cx', this.#fmtCoord(cx));
      ellipse.setAttribute('cy', this.#fmtCoord(cy));
      ellipse.setAttribute('rx', this.#fmtCoord(rx));
      ellipse.setAttribute('ry', this.#fmtCoord(ry));
      return ellipse;
    }

    #project({ x, y, z }) {
      const x3d = y;
      const y3d = -z;
      const z3d = x;
      const p = 4;
      const px = 1;
      const py = -1;
      const factor = p / (p - z3d);
      let sx = px + factor * (x3d - px);
      let sy = py + factor * (y3d - py);
      const len = Math.hypot(sx, sy);
      if (len > 1) {
        sx /= len;
        sy /= len;
      }
      return {
        x: CENTER + sx * RADIUS,
        y: CENTER + sy * RADIUS,
      };
    }

    #tipColor(z) {
      const t = Math.min(1, Math.max(0, Math.abs(z)));
      const target = z >= 0 ? COLORS.red300 : COLORS.blue300;
      const rgb = COLORS.purple400.map((mid, i) => Math.round(mid * (1 - t) + target[i] * t));
      return `rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`;
    }

    #buildPopover(vector) {
      const popover = document.createElement('div');
      popover.className = 'bd-popover';
      const polar = this.#polar(vector);
      popover.innerHTML = `
        <div class="bd-popover-title">Bloch sphere representation of local state</div>
        <div class="bd-popover-divider"></div>
        <div class="bd-popover-row">
          ${this.#popoverCell('r', this.#formatNumber(polar.r, 4))}
          ${this.#popoverCell('φ', polar.phi)}
          ${this.#popoverCell('θ', polar.theta)}
        </div>
        <div class="bd-popover-row">
          ${this.#popoverCell('x', this.#formatNumber(vector.x, 4))}
          ${this.#popoverCell('y', this.#formatNumber(vector.y, 4))}
          ${this.#popoverCell('z', this.#formatNumber(vector.z, 4))}
        </div>
        <svg class="bd-popover-tail" viewBox="0 0 16 8" aria-hidden="true">
          <path class="bd-popover-tail-fill" d="M0 0 L8 8 L16 0 Z"></path>
          <path class="bd-popover-tail-stroke" d="M0 0 L8 8 L16 0"></path>
        </svg>
      `;
      return popover;
    }

    #popoverCell(key, value) {
      return `<span class="bd-popover-cell"><span class="bd-popover-key">${key}</span><span class="bd-popover-val">${value}</span></span>`;
    }

    #polar(vector) {
      const r = this.#magnitude(vector);
      if (r < 1e-6) {
        return { r: 0, phi: '—', theta: '—' };
      }
      const phi = Math.atan2(vector.y, vector.x) * 180 / Math.PI;
      const theta = Math.acos(Math.min(1, Math.max(-1, vector.z / r))) * 180 / Math.PI;
      return {
        r,
        phi: `${this.#formatNumber(phi, 2)}°`,
        theta: `${this.#formatNumber(theta, 2)}°`,
      };
    }

    #magnitude({ x, y, z }) {
      return Math.hypot(x, y, z);
    }

    #formatNumber(value, digits) {
      const sign = value < 0 || Object.is(value, -0) ? '−' : '+';
      return `${sign}${Math.abs(value).toFixed(digits)}`;
    }

    #fmtCoord(value) {
      return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/0+$/, '').replace(/\.$/, '');
    }
  }

  if (!customElements.get('bloch-display')) {
    customElements.define('bloch-display', BlochDisplay);
  }
})();
