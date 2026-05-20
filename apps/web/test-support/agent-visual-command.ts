import { UI_CONSTANTS } from './generated-ui-constants'

type DragOperationInput = {
  cssWidth: number
  gate: string
  wire: string | number
  slot: string | number
  gateSize?: number
  paletteGap?: number
  paletteRowY?: number
  paletteRow1Count?: number
  paletteRowGap?: number
  paletteSectionGap?: number
  paletteSeparatorWidth?: number
  paletteDisplayColumns?: number
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

const DEFAULT_GATE_SIZE = UI_CONSTANTS.GATE_SIZE
const DEFAULT_PALETTE_GAP = UI_CONSTANTS.PALETTE_GAP
const DEFAULT_PALETTE_ROW_Y = UI_CONSTANTS.PALETTE_ROW_Y
const DEFAULT_PALETTE_ROW1_COUNT = 13
const DEFAULT_PALETTE_ROW_GAP = UI_CONSTANTS.PALETTE_ROW_GAP
const DEFAULT_PALETTE_SECTION_GAP = UI_CONSTANTS.PALETTE_SECTION_GAP
const DEFAULT_PALETTE_SEPARATOR_WIDTH = UI_CONSTANTS.PALETTE_SEPARATOR_WIDTH
const DEFAULT_PALETTE_DISPLAY_COLUMNS = UI_CONSTANTS.PALETTE_DISPLAY_COLUMNS
const DEFAULT_PALETTE_GATES_ROW2_INDICES = [13, 14, 15, 17, 18, 19, 22, 23, 21]
const DEFAULT_PALETTE_DISPLAY_INDICES = [16, 20, 24]
const DEFAULT_CIRCUIT_PADDING = UI_CONSTANTS.CIRCUIT_PADDING
const DEFAULT_QUBIT_LABEL_WIDTH = UI_CONSTANTS.QUBIT_LABEL_WIDTH
const DEFAULT_QUBIT_LABEL_GAP = UI_CONSTANTS.QUBIT_LABEL_GAP
const DEFAULT_LINE_Y = UI_CONSTANTS.LINE_Y
const DEFAULT_LINE_GAP = UI_CONSTANTS.LINE_GAP
const DEFAULT_SLOT_SPACING = UI_CONSTANTS.SLOT_SPACING

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
  ['probability', 20],
  ['probability-display', 20],
  ['probabilitydisplay', 20],
  ['probability_display', 20],
  ['probability', 20],
  ['probability-display', 20],
  ['probability_display', 20],
  ['spacer', 21],
  ['…', 21],
  ['...', 21],
  ['nop', 21],
  ['qft', 22],
  ['qft†', 23],
  ['qftdagger', 23],
  ['qft-dagger', 23],
  ['qft_dagger', 23],
  ['amps', 24],
  ['amplitude', 24],
  ['amplitude-display', 24],
  ['amplitudedisplay', 24],
  ['amplitude_display', 24],
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
  paletteRow1Count = DEFAULT_PALETTE_ROW1_COUNT,
  paletteRowGap = DEFAULT_PALETTE_ROW_GAP,
  paletteSectionGap = DEFAULT_PALETTE_SECTION_GAP,
  paletteSeparatorWidth = DEFAULT_PALETTE_SEPARATOR_WIDTH,
  paletteDisplayColumns = DEFAULT_PALETTE_DISPLAY_COLUMNS,
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
  const row1Width = paletteRow1Count > 0 ? paletteRow1Count * gateSize + (paletteRow1Count - 1) * paletteGap : 0
  const row2Count = DEFAULT_PALETTE_GATES_ROW2_INDICES.length
  const row2Width = row2Count > 0 ? row2Count * gateSize + (row2Count - 1) * paletteGap : 0
  const gatesWidth = Math.max(row1Width, row2Width)
  const displayWidth = paletteDisplayColumns * gateSize + (paletteDisplayColumns - 1) * paletteGap
  const displayX = gatesWidth + paletteSectionGap + paletteSeparatorWidth + paletteSectionGap
  const totalWidth = displayX + displayWidth
  const paletteStartX = Math.round(cssWidth / 2 - totalWidth / 2)
  const row2Col = DEFAULT_PALETTE_GATES_ROW2_INDICES.indexOf(gateIndex)
  const displaySlot = DEFAULT_PALETTE_DISPLAY_INDICES.indexOf(gateIndex)
  const paletteLocal = (() => {
    if (gateIndex < paletteRow1Count) {
      return { x: gateIndex * (gateSize + paletteGap), y: 0 }
    }
    if (row2Col >= 0) {
      return { x: row2Col * (gateSize + paletteGap), y: gateSize + paletteRowGap }
    }
    if (displaySlot >= 0) {
      return {
        x: displayX + (displaySlot % paletteDisplayColumns) * (gateSize + paletteGap),
        y: Math.floor(displaySlot / paletteDisplayColumns) * (gateSize + paletteRowGap),
      }
    }
    throw new Error(`Unknown palette gate index: ${gateIndex}`)
  })()
  const lineLeftOffset = circuitPadding + qubitLabelWidth + qubitLabelGap

  return {
    gate,
    gateIndex,
    wire: wireIndex,
    slot: slotIndex,
    from: {
      x: paletteStartX + paletteLocal.x + gateSize / 2,
      y: paletteRowY + paletteLocal.y + gateSize / 2 + verticalOffset,
    },
    to: {
      x: lineLeftOffset + gateSize + slotIndex * slotSpacing,
      y: lineY + wireIndex * lineGap + verticalOffset,
    },
  }
}
