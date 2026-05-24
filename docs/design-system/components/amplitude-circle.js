(() => {
  const ELEMENT_NAME = 'amplitude-circle';
  if (customElements.get(ELEMENT_NAME)) return;

  const clamp = (value, min, max) => Math.max(min, Math.min(max, value));
  const numberAttr = (el, name, fallback) => {
    const raw = el.getAttribute(name);
    if (raw === null || raw.trim() === '') return fallback;
    const value = Number(raw);
    return Number.isFinite(value) ? value : fallback;
  };
  const escapeAttr = (value) => String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('"', '&quot;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');

  class AmplitudeCircle extends HTMLElement {
    static get observedAttributes() {
      return ['coherent', 'label', 'mag', 'needle', 'phase', 'size'];
    }

    constructor() {
      super();
      this.attachShadow({ mode: 'open' });
    }

    connectedCallback() {
      this.render();
    }

    attributeChangedCallback() {
      this.render();
    }

    render() {
      if (!this.shadowRoot) return;

      const size = clamp(numberAttr(this, 'size', 40), 1, 256);
      const mag = clamp(numberAttr(this, 'mag', 1), 0, 1);
      const phaseDeg = numberAttr(this, 'phase', 0);
      const coherent = this.getAttribute('coherent') !== 'false';
      const showNeedle = coherent && this.getAttribute('needle') !== 'false' && mag >= 0.001 && size >= 12;
      const stroke = size >= 24 ? 2 : 1;
      const halfStroke = stroke / 2;
      const outlineRadius = Math.max(0, size / 2 - halfStroke - 1.5);
      const innerRadius = Math.max(0, outlineRadius - halfStroke);
      const diskRadius = mag * innerRadius;
      const center = size / 2;
      const phase = phaseDeg * Math.PI / 180;
      const x2 = center + (-Math.sin(phase) * innerRadius);
      const y2 = center + (-Math.cos(phase) * innerRadius);
      const outlineTone = mag > 0.001 ? 'var(--tx-2)' : 'var(--ui-2)';
      const diskFillOpacity = coherent ? '1' : '0.45';
      const label = this.getAttribute('label') ||
        `振幅円: 大きさ ${mag.toFixed(2)}, 位相 ${phaseDeg.toFixed(0)} 度`;

      this.shadowRoot.innerHTML = `
        <style>
          :host {
            display: inline-block;
            inline-size: ${size}px;
            block-size: ${size}px;
            line-height: 0;
          }
          svg {
            display: block;
            inline-size: 100%;
            block-size: 100%;
            overflow: visible;
          }
          .outline {
            fill: none;
            stroke: var(--amplitude-circle-outline, ${outlineTone});
            stroke-width: ${stroke};
          }
          .disk {
            fill: var(--blue-200); /* Flexoki blue-200: 振幅円盤 */
            fill-opacity: ${diskFillOpacity};
          }
          .disk-border {
            fill: none;
            stroke: var(--blue-400); /* Flexoki blue-400: 円盤境界線 */
            stroke-width: 1;
          }
          .needle {
            stroke: var(--tx); /* Flexoki tx: 位相針 */
            stroke-width: ${stroke};
            stroke-linecap: round;
          }
        </style>
        <svg viewBox="0 0 ${size} ${size}" role="img" aria-label="${escapeAttr(label)}">
          <circle class="outline" cx="${center}" cy="${center}" r="${outlineRadius}"></circle>
          ${diskRadius > 0.3 ? `<circle class="disk" cx="${center}" cy="${center}" r="${diskRadius}"></circle>` : ''}
          ${diskRadius >= 1.5 ? `<circle class="disk-border" cx="${center}" cy="${center}" r="${Math.max(0, diskRadius - 0.5)}"></circle>` : ''}
          ${showNeedle ? `<line class="needle" x1="${center}" y1="${center}" x2="${x2}" y2="${y2}"></line>` : ''}
        </svg>`;
    }
  }

  customElements.define(ELEMENT_NAME, AmplitudeCircle);
})();
