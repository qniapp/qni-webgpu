type DragOperationInput = {
  cssWidth: number
  gate: string
  wire: string | number
  slot: string | number
  gateSize?: number
  paletteGap?: number
  paletteRowY?: number
  paletteCount?: number
  circuitPadding?: number
  qubitLabelWidth?: number
  qubitLabelGap?: number
  lineY?: number
  lineGap?: number
  slotSpacing?: number
  verticalOffset?: number
}

type ScreenshotPlanInput = {
  command?: string
  out?: string
  canvasOut?: string
}

type ParsedOperation = {
  gate: string
  wire: string
  slot: number
}

const DEFAULT_REM = 32
const DEFAULT_GATE_SIZE = DEFAULT_REM
const DEFAULT_PALETTE_GAP = 0.5 * DEFAULT_REM
const DEFAULT_PALETTE_ROW_Y = 2 * DEFAULT_REM
const DEFAULT_PALETTE_COUNT = 15
const DEFAULT_CIRCUIT_PADDING = 2 * DEFAULT_REM
const DEFAULT_QUBIT_LABEL_WIDTH = 3 * 14
const DEFAULT_QUBIT_LABEL_GAP = 0.5 * DEFAULT_REM
const DEFAULT_LINE_Y = 6.5 * DEFAULT_REM
const DEFAULT_LINE_GAP = 1.5 * DEFAULT_REM
const DEFAULT_SLOT_SPACING = 1.5 * DEFAULT_GATE_SIZE

const GATE_ALIASES = new Map([
  ['h', 0],
  ['hadamard', 0],
  ['c', 1],
  ['control', 1],
  ['x', 2],
  ['y', 3],
  ['z', 4],
  ['sqrtx', 5],
  ['sqrt-x', 5],
  ['sqrt_x', 5],
  ['sx', 5],
  ['s', 6],
  ['sdg', 7],
  ['sdagger', 7],
  ['s†', 7],
  ['s+', 7],
  ['t', 8],
  ['tdg', 9],
  ['tdagger', 9],
  ['t†', 9],
  ['t+', 9],
  ['phase', 10],
  ['p', 10],
  ['rx', 11],
  ['ry', 12],
  ['rz', 13],
  ['swap', 14],
])

const normalizeGateName = (gate: unknown): string => String(gate || '').trim().toLowerCase()

export const getGateIndex = (gate: unknown): number => {
  const normalized = normalizeGateName(gate)
  if (!GATE_ALIASES.has(normalized)) {
    throw new Error(`Unknown gate: ${gate}`)
  }
  return GATE_ALIASES.get(normalized) as number
}

export const parseWire = (wire: unknown): number => {
  const normalized = String(wire ?? '').trim().toLowerCase().replace(/^q/, '')
  const parsed = Number.parseInt(normalized, 10)
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`Invalid wire: ${wire}`)
  }
  return parsed
}

export const parseSlot = (slot: unknown): number => {
  const parsed = Number.parseInt(String(slot ?? '').trim(), 10)
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`Invalid slot: ${slot}`)
  }
  return parsed
}

export const parseOperations = (ops: unknown): ParsedOperation[] =>
  String(ops || '')
    .split(',')
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [gate, wire, slot] = entry.split(':')
      if (!gate || !wire || slot === undefined) {
        throw new Error(`Invalid operation "${entry}". Use gate:wire:slot, for example H:q0:0.`)
      }
      return { gate, wire, slot: parseSlot(slot) }
    })

const defaultScreenshotPath = (command?: string): string =>
  `output/playwright/agent-visual/${command || 'screenshot'}.png`

export const buildScreenshotPlan = ({ command, out, canvasOut }: ScreenshotPlanInput = {}) => ({
  pageOut: out || defaultScreenshotPath(command),
  canvasOut: canvasOut || null,
})

export const buildDragOperation = ({
  cssWidth,
  gate,
  wire,
  slot,
  gateSize = DEFAULT_GATE_SIZE,
  paletteGap = DEFAULT_PALETTE_GAP,
  paletteRowY = DEFAULT_PALETTE_ROW_Y,
  paletteCount = DEFAULT_PALETTE_COUNT,
  circuitPadding = DEFAULT_CIRCUIT_PADDING,
  qubitLabelWidth = DEFAULT_QUBIT_LABEL_WIDTH,
  qubitLabelGap = DEFAULT_QUBIT_LABEL_GAP,
  lineY = DEFAULT_LINE_Y,
  lineGap = DEFAULT_LINE_GAP,
  slotSpacing = DEFAULT_SLOT_SPACING,
  verticalOffset = 0,
}: DragOperationInput) => {
  const gateIndex = getGateIndex(gate)
  const wireIndex = parseWire(wire)
  const slotIndex = parseSlot(slot)
  const paletteWidth = paletteCount * gateSize + (paletteCount - 1) * paletteGap
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const lineLeftOffset = circuitPadding + qubitLabelWidth + qubitLabelGap

  return {
    gate,
    gateIndex,
    wire: wireIndex,
    slot: slotIndex,
    from: {
      x: paletteStartX + gateIndex * (gateSize + paletteGap) + gateSize / 2,
      y: paletteRowY + gateSize / 2 + verticalOffset,
    },
    to: {
      x: lineLeftOffset + gateSize + slotIndex * slotSpacing,
      y: lineY + wireIndex * lineGap + verticalOffset,
    },
  }
}
