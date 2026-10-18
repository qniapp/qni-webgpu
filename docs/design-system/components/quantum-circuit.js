/* ─────────────────────────────────────────────────────────────────────
 * quantum-circuit.js — <quantum-circuit> カスタム要素
 *
 * Quirk 互換の cols JSON（`[["H"],["•","X"], …]` または `{"cols":[…]}`）を
 * 受け取り、回路全体（量子ビットワイヤ・ゲート・縦跨ぎ QFT/表示ブロック・
 * 制御線）を組み立てて描く。Qni（~/Work/qni packages/elements の
 * quantum-circuit-element + circuit-step-element）の「データから回路を描く」
 * 方式を踏襲し、部品には設計システムの既存カスタム要素を使う:
 *   - 1 セル（ワイヤ＋制御線）= circuit-dropzone
 *   - 単一ワイヤゲート = h-gate / x-gate / control-gate / … 各要素
 *   - 縦跨ぎ = qft-gate（span 属性）/ probability-display（placeholder ボックス）ほか
 *
 * P(角度) に対応。未対応: 回転ゲート Rx / Ry / Rz、測定変数。
 * 表示ブロックは計算結果ではなく配置を示す箱として描く。
 *
 * 図とサンプル定義が常に一致するよう、サンプルと同じ cols JSON を入力に取る。
 *
 * 使い方:
 *   <script src="components/circuit-dropzone.js"></script>  <!-- 依存（先に読み込む） -->
 *   <script src="components/h-gate.js"></script> …          <!-- 使うゲートの要素 -->
 *   <script src="components/quantum-circuit.js"></script>
 *
 *   <quantum-circuit cols='[["H",1,"H"],["•","X"],[1,1,"•","X"]]'></quantum-circuit>
 *
 * 制御線の計算: 各列で、制御（•/◦）がある場合は制御＋ターゲットを 1 本の縦線で
 * 繋ぐ。SWAP は 2 つ以上を繋ぐ。縦跨ぎゲートは「上端ワイヤ」を接続点として扱い、
 * 箱の本体（下のワイヤ）はオーバーレイが覆う。
 * ───────────────────────────────────────────────────────────────────── */
(function () {
  'use strict';

  if (customElements.get('quantum-circuit')) return;

  const CELL = 56; // SLOT_SPACING / LINE_GAP = spacing-14

  // token → 単一ワイヤゲートのタグ名
  const SINGLE = {
    '•': 'control-gate',
    '◦': 'anti-control-gate',
    H: 'h-gate',
    X: 'x-gate',
    Y: 'y-gate',
    Z: 'z-gate',
    'X^½': 'sqrtx-gate',
    S: 's-gate',
    'S†': 's-dagger-gate',
    T: 't-gate',
    'T†': 't-dagger-gate',
    Swap: 'swap-gate',
    '|0>': 'write0-gate',
    '|1>': 'write1-gate',
    Measure: 'measurement-gate',
    '…': 'spacer-gate',
    Bloch: 'bloch-display',
  };

  // 縦跨ぎ（span を持つ）ゲート / 表示ブロックの token → タグ名と span
  function spanningOf(token) {
    let m;
    if ((m = /^QFT†(\d+)$/.exec(token))) return { tag: 'qft-dagger-gate', span: Number(m[1]) };
    if ((m = /^QFT(\d+)$/.exec(token))) return { tag: 'qft-gate', span: Number(m[1]) };
    if ((m = /^Probability(\d*)$/.exec(token))) return { tag: 'probability-display', span: Number(m[1] || 1) };
    if ((m = /^Amps(\d+)$/.exec(token))) return { tag: 'amplitude-display', span: Number(m[1]) };
    if ((m = /^Density(\d*)$/.exec(token))) return { tag: 'density-matrix-display', span: Number(m[1] || 1) };
    if ((m = /^Sample([1-3])$/.exec(token))) return { tag: 'div', span: Number(m[1]), sample: true };
    return null;
  }

  function isEmpty(tok) {
    return tok === 1 || tok === 0 || tok === null || tok === undefined || tok === '';
  }

  class QuantumCircuit extends HTMLElement {
    static get observedAttributes() {
      return ['cols'];
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

    #parseCols() {
      try {
        const parsed = JSON.parse(this.getAttribute('cols') || '[]');
        return Array.isArray(parsed) ? parsed : parsed.cols || [];
      } catch {
        return [];
      }
    }

    #render() {
      const cols = [];
      const blocks = [];
      let open = null;
      const close = () => {
        if (open && cols.length > open.start) blocks.push({ ...open, end: cols.length });
        open = null;
      };
      for (const col of this.#parseCols()) {
        const token = col?.[0];
        if (col?.length === 1 && typeof token === 'string' && token.startsWith('{')) {
          // 不正な入れ子や空ラベルは回路全体を表示しない。
          if (open || token.length === 1) { this.shadowRoot.replaceChildren(); return; }
          open = { label: token.slice(1), start: cols.length };
        } else if (col?.length === 1 && token === '}') {
          if (!open) { this.shadowRoot.replaceChildren(); return; }
          close();
        } else {
          cols.push(col);
        }
      }
      close(); // アプリと同様、閉じ忘れは末尾で閉じ、空ブロックは捨てる。

      // 量子ビット数 = 各セルの (wire + span) の最大値
      let qubits = 1;
      cols.forEach((col) =>
        (col || []).forEach((tok, w) => {
          if (isEmpty(tok)) return;
          const sp = typeof tok === 'string' ? spanningOf(tok) : null;
          qubits = Math.max(qubits, w + (sp ? sp.span : 1));
        }),
      );

      // 制御線フラグ connect[c][r] = {top, bottom}
      const connect = cols.map(() => Array.from({ length: qubits }, () => ({ top: false, bottom: false })));
      const link = (c, top, bottom) => {
        for (let r = top; r <= bottom; r++) {
          if (r > top) connect[c][r].top = true;
          if (r < bottom) connect[c][r].bottom = true;
        }
      };
      cols.forEach((col, c) => {
        const controls = [];
        const targets = []; // 接続点（単一ゲート、または縦跨ぎゲートの上端ワイヤ）
        const swaps = [];
        (col || []).forEach((tok, w) => {
          if (isEmpty(tok) || typeof tok !== 'string') return;
          if (tok === '•' || tok === '◦') controls.push(w);
          else if (tok === '…') return; // 区切りは繋がない
          else if (tok === 'Swap') swaps.push(w);
          else targets.push(w);
        });
        if (controls.length > 0) {
          // 制御つき操作（controlled-U / Fredkin）。制御・ターゲット・SWAP を 1 本で繋ぐ。
          const group = controls.concat(targets, swaps);
          link(c, Math.min(...group), Math.max(...group));
        } else if (swaps.length >= 2) {
          link(c, Math.min(...swaps), Math.max(...swaps));
        }
      });

      const style = document.createElement('style');
      style.textContent = `
        :host { display: block; }
        .grid {
          display: grid;
          grid-template-columns: max-content repeat(${cols.length}, ${CELL}px);
          grid-auto-rows: ${CELL}px;
          width: max-content;
          padding-block: ${blocks.length ? 58 : 0}px;
          padding-inline-end: ${blocks.length ? 16 : 0}px;
          --ac-mask: var(--bg); /* 反制御の空円内側を紙色で消し込む */
        }
        .circuit-block {
          background: color-mix(in srgb, var(--ui) 50%, transparent);
          border-block: 2px solid var(--blue-400);
          margin-block: -34px; /* アプリの上下 padding 32px + 罫線 2px */
          pointer-events: none;
        }
        circuit-dropzone { z-index: 1; }
        .block-label {
          justify-self: center;
          font: 16px/28px var(--font-mono);
          color: var(--blue-400);
          white-space: nowrap;
          pointer-events: none;
        }
        .qubit-label {
          grid-column: 1;
          align-self: center;
          text-align: right;
          padding-right: 8px; /* spacing-2 */
          font-family: var(--font-mono);
          font-size: 12px; /* text-xs */
          line-height: 16px;
          color: var(--tx-2); /* Flexoki base-600 */
          white-space: nowrap;
        }
        .span-gate { align-self: center; justify-self: center; z-index: 20; }
        .sample-placeholder {
          display: flex; flex-direction: column; gap: 16px;
          width: 40px; background: var(--purple-100); color: var(--tx-3);
          outline: 1px solid var(--ui-2); text-align: center;
          font: 16px/40px var(--font-mono);
        }
        .angle-gate { width: 40px; height: 40px; }
        .angle-label {
          position: absolute; top: -20px; left: 50%; transform: translateX(-50%);
          font: 12px/16px var(--font-mono); color: var(--tx); white-space: nowrap;
        }
      `;

      const grid = document.createElement('div');
      grid.className = 'grid';

      for (const block of blocks) {
        const column = `${block.start + 2} / span ${block.end - block.start}`;
        const row = `1 / span ${qubits}`;
        const band = document.createElement('div');
        band.className = 'circuit-block';
        band.style.gridColumn = column;
        band.style.gridRow = row;
        grid.appendChild(band);
        for (const edge of ['start', 'end']) {
          const label = document.createElement('div');
          label.className = 'block-label';
          label.style.gridColumn = column;
          label.style.gridRow = row;
          label.style.alignSelf = edge;
          label.style.transform = `translateY(${edge === 'start' ? -58 : 58}px)`;
          label.textContent = block.label;
          if (edge === 'end') label.setAttribute('aria-hidden', 'true');
          grid.appendChild(label);
        }
      }

      // 量子ビットラベル
      for (let r = 0; r < qubits; r++) {
        const label = document.createElement('div');
        label.className = 'qubit-label';
        label.style.gridRow = String(r + 1);
        label.textContent = `q${r}`;
        grid.appendChild(label);
      }

      // セル（circuit-dropzone）＋ 縦跨ぎオーバーレイ
      cols.forEach((col, c) => {
        col = col || [];
        for (let r = 0; r < qubits; r++) {
          const cell = document.createElement('circuit-dropzone');
          cell.style.gridColumn = String(c + 2);
          cell.style.gridRow = String(r + 1);
          if (connect[c][r].top) cell.setAttribute('connect-top', '');
          if (connect[c][r].bottom) cell.setAttribute('connect-bottom', '');
          const tok = col[r];
          const phase = typeof tok === 'string' ? /^P\((.+)\)$/.exec(tok) : null;
          if (phase) {
            const gate = document.createElement('div');
            gate.className = 'angle-gate';
            gate.appendChild(document.createElement('p-gate'));
            const label = document.createElement('span');
            label.className = 'angle-label';
            label.textContent = phase[1].replaceAll('_', '/');
            gate.appendChild(label);
            cell.appendChild(gate);
          } else if (typeof tok === 'string' && !spanningOf(tok) && SINGLE[tok]) {
            cell.appendChild(document.createElement(SINGLE[tok]));
          }
          grid.appendChild(cell);
        }
        col.forEach((tok, r) => {
          if (typeof tok !== 'string') return;
          const sp = spanningOf(tok);
          if (!sp) return;
          const span = Math.max(1, sp.span);
          const box = document.createElement(sp.tag);
          box.className = sp.sample ? 'span-gate sample-placeholder' : 'span-gate';
          const width = sp.tag === 'amplitude-display'
            ? (span === 1 ? 2 : span % 2 === 0 ? span : Math.ceil(span / 2))
            : 1;
          box.style.gridColumn = `${c + 2} / span ${width}`;
          box.style.gridRow = `${r + 1} / span ${span}`;
          if (!sp.sample) box.setAttribute('span', String(span));
          if (sp.sample) {
            for (let wire = 0; wire < span; wire++) {
              const value = document.createElement('div');
              value.textContent = '?';
              box.appendChild(value);
            }
          }
          if (!sp.sample && sp.tag !== 'qft-gate' && sp.tag !== 'qft-dagger-gate') {
            // 表示ブロックは中身を計算せず、箱（placeholder）だけ置く
            box.setAttribute('placeholder', '');
            box.setAttribute('height', String(CELL * span - 16));
          }
          grid.appendChild(box);
        });
      });

      this.shadowRoot.replaceChildren(style, grid);
    }
  }

  customElements.define('quantum-circuit', QuantumCircuit);
})();
