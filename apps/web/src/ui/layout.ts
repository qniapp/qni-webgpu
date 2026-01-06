import { COLORS, GATE_SIZE, GATE_ICON_CHAR_MAP, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE, getLayoutMetrics } from './constants'
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
  wireLabels: TextLayout[]
  stateVector: TextLayout
}

const labelPosition = (x: number, y: number, size: number, text: string) => {
  const width = text.length * LABEL_GLYPH_SIZE
  return {
    x: x + (size - width) / 2,
    y: y + (size - LABEL_GLYPH_SIZE) / 2,
  }
}

export function buildScene(
  stateVectorGlyphCount: number,
  placedGates: PlacedGate[],
  canvasWidth: number,
  canvasHeight: number,
  hoveredPaletteIndex: number | null
): SceneLayout {
  instances.length = 0
  const metrics = getLayoutMetrics(canvasWidth)
  metrics.lineYs.forEach((lineY) => {
    addLine(metrics.lineLeft, lineY, metrics.lineRight, lineY, 2, COLORS.line)
  })

  const paletteWidth = PALETTE_GATES.length * PALETTE_SIZE + (PALETTE_GATES.length - 1) * PALETTE_GAP
  const paletteStartX = (canvasWidth - paletteWidth) / 2
  const paletteLabels: TextLayout[] = []
  PALETTE_GATES.forEach((gate, index) => {
    const x = paletteStartX + index * (PALETTE_SIZE + PALETTE_GAP)
    if (hoveredPaletteIndex === index) {
      addRoundedRect(x - 4, PALETTE_ROW_Y - 4, PALETTE_SIZE + 8, PALETTE_SIZE + 8, 10, COLORS.boxBorder)
      addRoundedRect(x - 2, PALETTE_ROW_Y - 2, PALETTE_SIZE + 4, PALETTE_SIZE + 4, 8, COLORS.background)
    }
    addRoundedRect(x, PALETTE_ROW_Y, PALETTE_SIZE, PALETTE_SIZE, 6, COLORS.box)
    const paletteLabel = GATE_ICON_CHAR_MAP[gate]
    const palettePos = labelPosition(x, PALETTE_ROW_Y, PALETTE_SIZE, paletteLabel)
    paletteLabels.push({
      text: paletteLabel,
      x: palettePos.x,
      y: palettePos.y,
      color: COLORS.label,
    })
  })

  const gateLabels: TextLayout[] = []
  placedGates.forEach((gate) => {
    if (!gate.dragging) {
      if (gate.hovered) {
        addRoundedRect(gate.x - 4, gate.y - 4, GATE_SIZE + 8, GATE_SIZE + 8, 10, COLORS.boxBorder)
        addRoundedRect(gate.x - 2, gate.y - 2, GATE_SIZE + 4, GATE_SIZE + 4, 8, COLORS.background)
      }
      addRoundedRect(gate.x, gate.y, GATE_SIZE, GATE_SIZE, 6, COLORS.box)
    }
    const gateLabel = GATE_ICON_CHAR_MAP[gate.label]
    const gatePos = labelPosition(gate.x, gate.y, GATE_SIZE, gateLabel)
    gateLabels.push({
      text: gateLabel,
      x: gatePos.x,
      y: gatePos.y,
      color: COLORS.label,
    })
  })

  const wireLabels: TextLayout[] = metrics.lineYs.map((lineY, index) => ({
    text: `q${index}:`,
    x: metrics.lineLeft - FONT_GLYPH_SIZE * 3 - 12,
    y: lineY - FONT_GLYPH_SIZE / 2,
    color: COLORS.text,
  }))

  const stateVectorWidth = stateVectorGlyphCount * FONT_GLYPH_SIZE
  const stateVectorX = (canvasWidth - stateVectorWidth) / 2
  const stateVectorY = canvasHeight - 40 - FONT_GLYPH_SIZE

  return {
    instances: [...instances],
    gateLabels,
    paletteLabels,
    wireLabels,
    stateVector: {
      text: '',
      x: stateVectorX,
      y: stateVectorY,
      color: COLORS.text,
      glyphCount: stateVectorGlyphCount,
    },
  }
}
