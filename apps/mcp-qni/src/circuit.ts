const GATES = ['X', 'H', 'Y', 'Z', 'S', 'T'] as const
const GATE_SET = new Set<string>(GATES)
const INV_SQRT2 = 1 / Math.sqrt(2)

export type Gate = (typeof GATES)[number]

export type Complex = {
  re: number
  im: number
}

export type Operation = {
  gate: Gate
  target: number
  column: number
  order: number
}

export type Circuit = {
  qubits: number
  operations: Operation[]
  _order: number
}

type Matrix = [[Complex, Complex], [Complex, Complex]]

function complex(re: number, im: number): Complex {
  return { re, im }
}

const COMPLEX_ZERO = complex(0, 0)
const COMPLEX_ONE = complex(1, 0)
const COMPLEX_I = complex(0, 1)
const COMPLEX_NEG_I = complex(0, -1)

const GATE_MATRICES: Record<Gate, Matrix> = {
  X: [
    [COMPLEX_ZERO, COMPLEX_ONE],
    [COMPLEX_ONE, COMPLEX_ZERO],
  ],
  H: [
    [complex(INV_SQRT2, 0), complex(INV_SQRT2, 0)],
    [complex(INV_SQRT2, 0), complex(-INV_SQRT2, 0)],
  ],
  Y: [
    [COMPLEX_ZERO, COMPLEX_NEG_I],
    [COMPLEX_I, COMPLEX_ZERO],
  ],
  Z: [
    [COMPLEX_ONE, COMPLEX_ZERO],
    [COMPLEX_ZERO, complex(-1, 0)],
  ],
  S: [
    [COMPLEX_ONE, COMPLEX_ZERO],
    [COMPLEX_ZERO, COMPLEX_I],
  ],
  T: [
    [COMPLEX_ONE, COMPLEX_ZERO],
    [COMPLEX_ZERO, complex(INV_SQRT2, INV_SQRT2)],
  ],
}

function complexAdd(a: Complex, b: Complex): Complex {
  return { re: a.re + b.re, im: a.im + b.im }
}

function complexMul(a: Complex, b: Complex): Complex {
  return { re: a.re * b.re - a.im * b.im, im: a.re * b.im + a.im * b.re }
}

function normalizeGate(value: unknown): Gate | null {
  if (typeof value !== 'string') {
    return null
  }
  const gate = value.trim().toUpperCase()
  return GATE_SET.has(gate) ? (gate as Gate) : null
}

function requirePositiveInt(value: number, label: string): void {
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`${label} must be an integer >= 1`)
  }
}

export function createCircuit(qubits = 1): Circuit {
  requirePositiveInt(qubits, 'qubits')
  return { qubits, operations: [], _order: 0 }
}

export function setQubits(circuit: Circuit, qubits: number): void {
  requirePositiveInt(qubits, 'qubits')
  circuit.qubits = qubits
  circuit.operations = []
  circuit._order = 0
}

export function clearCircuit(circuit: Circuit): void {
  circuit.operations = []
  circuit._order = 0
}

export function placeGate(
  circuit: Circuit,
  {
    gate,
    target,
    column,
  }: {
    gate: unknown
    target: number
    column: number
  }
): void {
  const normalizedGate = normalizeGate(gate)
  if (!normalizedGate) {
    throw new Error('gate must be one of X, H, Y, Z, S, T')
  }
  if (!Number.isInteger(target) || target < 0 || target >= circuit.qubits) {
    throw new Error('target must be a valid qubit index')
  }
  if (!Number.isInteger(column) || column < 0) {
    throw new Error('column must be an integer >= 0')
  }

  const existingIndex = circuit.operations.findIndex(
    (operation) => operation.target === target && operation.column === column
  )
  if (existingIndex >= 0) {
    circuit.operations[existingIndex].gate = normalizedGate
    return
  }

  circuit.operations.push({
    gate: normalizedGate,
    target,
    column,
    order: circuit._order,
  })
  circuit._order += 1
}

function createZeroState(qubits: number): Complex[] {
  const size = 1 << qubits
  const state = Array.from({ length: size }, () => ({ re: 0, im: 0 }))
  state[0] = { re: 1, im: 0 }
  return state
}

function applyGate(state: Complex[], target: number, gate: Gate): void {
  const matrix = GATE_MATRICES[gate]
  const step = 1 << target
  const span = step * 2
  for (let base = 0; base < state.length; base += span) {
    for (let offset = 0; offset < step; offset += 1) {
      const index0 = base + offset
      const index1 = index0 + step
      const a0 = state[index0]
      const a1 = state[index1]
      const m00 = matrix[0][0]
      const m01 = matrix[0][1]
      const m10 = matrix[1][0]
      const m11 = matrix[1][1]
      const new0 = complexAdd(complexMul(m00, a0), complexMul(m01, a1))
      const new1 = complexAdd(complexMul(m10, a0), complexMul(m11, a1))
      state[index0] = new0
      state[index1] = new1
    }
  }
}

export function runCircuit(circuit: Circuit): Complex[] {
  const state = createZeroState(circuit.qubits)
  const ordered = [...circuit.operations].sort((a, b) => {
    if (a.column !== b.column) {
      return a.column - b.column
    }
    return a.order - b.order
  })

  ordered.forEach((operation) => {
    applyGate(state, operation.target, operation.gate)
  })

  return state
}
