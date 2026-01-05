import { COLORS, CANVAS_HEIGHT, CANVAS_WIDTH, GATE_SIZE, LINE_LEFT, LINE_RIGHT, LINE_Y, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE } from './constants'
import { FONT_GLYPH_SIZE, LABEL_GLYPH_SIZE } from './text'
import type { PlacedGate, ShapeInstance, TextLayout } from './types'

const instances: ShapeInstance[] = []

const addRoundedRect = (x: number, y: number, w: number, h: number, radius: number, color: ShapeInstance['color']) => {
  instances.push({
    kind: 2,
    thickness: radius,
    p0x: x,
    p0y: y,
    p1x: w,
    p1y: h,
    color,
  })
}

const addLine = (x1: number, y1: number, x2: number, y2: number, thickness: number, color: ShapeInstance['color']) => {
  instances.push({
    kind: 1,
    thickness,
    p0x: x1,
    p0y: y1,
    p1x: x2,
    p1y: y2,
    color,
  })
}

export type SceneLayout = {
  instances: ShapeInstance[]
  gateLabels: TextLayout[]
  paletteLabels: TextLayout[]
  stateVector: TextLayout
}

export function buildScene(stateVectorGlyphCount: number, placedGates: PlacedGate[]): SceneLayout {
  instances.length = 0
  addLine(LINE_LEFT, LINE_Y, LINE_RIGHT, LINE_Y, 4, COLORS.line)

  const paletteWidth = PALETTE_GATES.length * PALETTE_SIZE + (PALETTE_GATES.length - 1) * PALETTE_GAP
  const paletteStartX = (CANVAS_WIDTH - paletteWidth) / 2
  const paletteLabels: TextLayout[] = []
  PALETTE_GATES.forEach((gate, index) => {
    const x = paletteStartX + index * (PALETTE_SIZE + PALETTE_GAP)
    addRoundedRect(x, PALETTE_ROW_Y, PALETTE_SIZE, PALETTE_SIZE, 6, COLORS.box)
    paletteLabels.push({
      text: gate,
      x: x + PALETTE_SIZE / 2 - LABEL_GLYPH_SIZE / 2,
      y: PALETTE_ROW_Y + PALETTE_SIZE / 2 - LABEL_GLYPH_SIZE / 2,
      color: COLORS.label,
    })
  })

  const gateLabels: TextLayout[] = []
  placedGates.forEach((gate) => {
    if (!gate.dragging) {
      addRoundedRect(gate.x, gate.y, GATE_SIZE, GATE_SIZE, 6, COLORS.box)
    }
    gateLabels.push({
      text: gate.label,
      x: gate.x + GATE_SIZE / 2 - LABEL_GLYPH_SIZE / 2,
      y: gate.y + GATE_SIZE / 2 - LABEL_GLYPH_SIZE / 2,
      color: COLORS.label,
    })
  })

  const stateVectorWidth = stateVectorGlyphCount * FONT_GLYPH_SIZE
  const stateVectorX = (CANVAS_WIDTH - stateVectorWidth) / 2
  const stateVectorY = CANVAS_HEIGHT - 40 - FONT_GLYPH_SIZE

  return {
    instances: [...instances],
    gateLabels,
    paletteLabels,
    stateVector: {
      text: '',
      x: stateVectorX,
      y: stateVectorY,
      color: COLORS.text,
      glyphCount: stateVectorGlyphCount,
    },
  }
}
