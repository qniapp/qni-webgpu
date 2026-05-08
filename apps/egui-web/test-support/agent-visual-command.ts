type DragOperationInput = {
  cssWidth: number
  gate: string
  wire: string | number
  slot: string | number
  gateSize?: number
  paletteGap?: number
  paletteRowY?: number
  paletteCount?: number
  paletteRow1Count?: number
  paletteRowGap?: number
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
const DEFAULT_PALETTE_GAP = 8
const DEFAULT_PALETTE_ROW_Y = 2 * DEFAULT_REM
const DEFAULT_PALETTE_COUNT = 20
const DEFAULT_PALETTE_ROW1_COUNT = 13
const DEFAULT_PALETTE_ROW_GAP = 8
const DEFAULT_CIRCUIT_PADDING = 2 * DEFAULT_REM
const DEFAULT_QUBIT_LABEL_WIDTH = 3 * 14
const DEFAULT_QUBIT_LABEL_GAP = 0.5 * DEFAULT_REM
const DEFAULT_LINE_Y = 6.5 * DEFAULT_REM
const DEFAULT_LINE_GAP = 1.5 * DEFAULT_REM
const DEFAULT_SLOT_SPACING = 1.5 * DEFAULT_GATE_SIZE

const GATE_ALIASES = new Map([
  ['h', 0],
  ['hadamard', 0],
  ['x', 1],
  ['y', 2],
  ['z', 3],
  ['sqrtx', 4],
  ['sqrt-x', 4],
  ['sqrt_x', 4],
  ['sx', 4],
  ['s', 5],
  ['sdg', 6],
  ['sdagger', 6],
  ['s†', 6],
  ['s+', 6],
  ['t', 7],
  ['tdg', 8],
  ['tdagger', 8],
  ['t†', 8],
  ['t+', 8],
  ['phase', 9],
  ['p', 9],
  ['rx', 10],
  ['ry', 11],
  ['rz', 12],
  ['swap', 13],
  ['c', 14],
  ['control', 14],
  ['anti', 15],
  ['anti-control', 15],
  ['anticontrol', 15],
  ['anti_control', 15],
  ['o', 15],
  ['◦', 15],
  ['bloch', 16],
  ['bloch-display', 16],
  ['blochdisplay', 16],
  ['bloch_display', 16],
  ['sphere', 16],
  ['|0>', 17],
  ['|0⟩', 17],
  ['write0', 17],
  ['write-0', 17],
  ['write_0', 17],
  ['ket0', 17],
  ['ket-0', 17],
  ['|1>', 18],
  ['|1⟩', 18],
  ['write1', 18],
  ['write-1', 18],
  ['write_1', 18],
  ['ket1', 18],
  ['ket-1', 18],
  ['m', 19],
  ['measure', 19],
  ['measurement', 19],
  ['meter', 19],
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
  paletteRow1Count = DEFAULT_PALETTE_ROW1_COUNT,
  paletteRowGap = DEFAULT_PALETTE_ROW_GAP,
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
  const row2Count = Math.max(paletteCount - paletteRow1Count, 0)
  const row1Width = paletteRow1Count > 0 ? paletteRow1Count * gateSize + (paletteRow1Count - 1) * paletteGap : 0
  const row2Width = row2Count > 0 ? row2Count * gateSize + (row2Count - 1) * paletteGap : 0
  const totalWidth = Math.max(row1Width, row2Width)
  const paletteStartX = cssWidth / 2 - totalWidth / 2
  const row = gateIndex < paletteRow1Count ? 0 : 1
  const col = gateIndex < paletteRow1Count ? gateIndex : gateIndex - paletteRow1Count
  const lineLeftOffset = circuitPadding + qubitLabelWidth + qubitLabelGap

  return {
    gate,
    gateIndex,
    wire: wireIndex,
    slot: slotIndex,
    from: {
      // Both rows are left-aligned to match qni's `flex flex-row` layout.
      x: paletteStartX + col * (gateSize + paletteGap) + gateSize / 2,
      y: paletteRowY + row * (gateSize + paletteRowGap) + gateSize / 2 + verticalOffset,
    },
    to: {
      x: lineLeftOffset + gateSize + slotIndex * slotSpacing,
      y: lineY + wireIndex * lineGap + verticalOffset,
    },
  }
}
