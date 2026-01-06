import type { Gate } from '../domain/gate'
import type { Color } from './types'

export const REM = 32
export const DEFAULT_CANVAS_WIDTH = 25 * REM
export const DEFAULT_CANVAS_HEIGHT = 18.75 * REM
export const STATE_CIRCLE_COUNT = 4
export const STATE_CIRCLE_SIZE = 1 * REM
export const STATE_CIRCLE_GAP = 0.5 * REM
export const STATE_CIRCLE_BOTTOM_MARGIN = 2 * REM
export const STATE_CIRCLE_STROKE = 2
export const STATE_CIRCLE_INSTANCE_COUNT = STATE_CIRCLE_COUNT * 4

export const LINE_Y = 6.5 * REM
export const LINE_GAP = 1.5 * REM
export const LINE_Y_VALUES = [LINE_Y, LINE_Y + LINE_GAP]
export const LINE_LEFT_OFFSET = 2 * REM
export const LINE_RIGHT_OFFSET = 2 * REM

export const GATE_SIZE = 1 * REM

export const SLOT_SPACING = GATE_SIZE * 1.5
export const SNAP_DISTANCE = 0.5625 * REM

export const PALETTE_GATES: Gate[] = ['H', 'X', 'Y', 'Z', '√X', 'S', 'S†', 'T', 'T†']
export const PALETTE_SIZE = GATE_SIZE
export const PALETTE_GAP = 0.5 * REM
export const PALETTE_ROW_Y = 2 * REM

export const GATE_ICON_CHAR_MAP: Record<Gate, string> = {
  H: 'A',
  X: 'B',
  Y: 'C',
  Z: 'D',
  '√X': 'E',
  S: 'F',
  'S†': 'G',
  T: 'H',
  'T†': 'I',
}

export type LayoutMetrics = {
  lineLeft: number
  lineRight: number
  lineYs: number[]
  slotLeft: number
  slotRight: number
  slotSpacing: number
  slotCount: number
  slotCenters: number[]
}

export function getLayoutMetrics(canvasWidth: number): LayoutMetrics {
  const lineLeft = LINE_LEFT_OFFSET
  const lineRight = canvasWidth - LINE_RIGHT_OFFSET
  const lineYs = [LINE_Y, LINE_Y + LINE_GAP]
  const slotLeft = lineLeft + GATE_SIZE
  const slotRight = lineRight - GATE_SIZE
  const slotSpacing = SLOT_SPACING
  const slotCount = slotSpacing > 0 ? Math.floor((slotRight - slotLeft) / slotSpacing) + 1 : 0
  const slotCenters = Array.from({ length: slotCount }, (_, index) => slotLeft + slotSpacing * index)
  return { lineLeft, lineRight, lineYs, slotLeft, slotRight, slotSpacing, slotCount, slotCenters }
}

export const COLORS: Record<string, Color> = {
  background: [0.976, 0.98, 0.984, 1.0],
  surface: [1.0, 1.0, 1.0, 1.0],
  line: [0.72, 0.72, 0.72, 1.0],
  box: [0.2, 0.62, 0.55, 1.0],
  boxBorder: [0.82, 0.82, 0.82, 1.0],
  boxActive: [0.52, 0.45, 0.7, 1.0],
  label: [1.0, 1.0, 1.0, 1.0],
  text: [0.45, 0.45, 0.45, 1.0],
  stateFill: [0.16, 0.58, 0.78, 1.0],
  stateOutline: [0.16, 0.58, 0.78, 1.0],
  stateOutlineZero: [0.75, 0.75, 0.75, 1.0],
  stateNeedle: [0.0, 0.0, 0.0, 1.0],
}
