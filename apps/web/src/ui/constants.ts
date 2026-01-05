import type { Gate } from '../domain/gate'
import type { Color } from './types'

export const CANVAS_WIDTH = 800
export const CANVAS_HEIGHT = 600
export const STATE_TEXT_MAX_LEN = 50

export const LINE_Y = 160
export const LINE_LEFT = 80
export const LINE_RIGHT = CANVAS_WIDTH - 80

export const GATE_SIZE = 60

export const PALETTE_GATES: Gate[] = ['X', 'H', 'Y', 'Z', 'S', 'T']
export const PALETTE_SIZE = 60
export const PALETTE_GAP = 16
export const PALETTE_ROW_Y = 12

export const COLORS: Record<string, Color> = {
  background: [0.94, 0.94, 0.94, 1.0],
  line: [0.62, 0.62, 0.62, 1.0],
  box: [0.2, 0.62, 0.55, 1.0],
  boxBorder: [0.14, 0.36, 0.34, 1.0],
  boxActive: [0.52, 0.45, 0.7, 1.0],
  label: [1.0, 1.0, 1.0, 1.0],
  text: [0.45, 0.45, 0.45, 1.0],
}
