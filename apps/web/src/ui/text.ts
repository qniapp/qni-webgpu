export const FONT_GLYPH_SIZE = 8
export const LABEL_GLYPH_SIZE = 14
export const FONT_COLS = 16
export const FONT_ROWS = 6

export type GlyphMap = Record<string, string[]>

export const BASE_GLYPHS: GlyphMap = {
  H: [
    '11000011',
    '11000011',
    '11000011',
    '11111111',
    '11000011',
    '11000011',
    '11000011',
    '00000000',
  ],
  X: [
    '11000011',
    '01100110',
    '00111100',
    '00011000',
    '00111100',
    '01100110',
    '11000011',
    '00000000',
  ],
  Y: [
    '11000011',
    '01100110',
    '00111100',
    '00011000',
    '00011000',
    '00011000',
    '00011000',
    '00000000',
  ],
  Z: [
    '11111111',
    '00000110',
    '00001100',
    '00011000',
    '00110000',
    '01100000',
    '11111111',
    '00000000',
  ],
  S: [
    '01111110',
    '11000000',
    '11000000',
    '01111100',
    '00000011',
    '00000011',
    '11111110',
    '00000000',
  ],
  T: [
    '11111111',
    '00011000',
    '00011000',
    '00011000',
    '00011000',
    '00011000',
    '00011000',
    '00000000',
  ],
  '0': [
    '00111100',
    '01100110',
    '01101110',
    '01110110',
    '01100110',
    '01100110',
    '00111100',
    '00000000',
  ],
  '1': [
    '00011000',
    '00111000',
    '00011000',
    '00011000',
    '00011000',
    '00011000',
    '00111100',
    '00000000',
  ],
  '2': [
    '00111100',
    '01100110',
    '00000110',
    '00001100',
    '00011000',
    '00110000',
    '01111110',
    '00000000',
  ],
  '3': [
    '00111100',
    '01100110',
    '00000110',
    '00011100',
    '00000110',
    '01100110',
    '00111100',
    '00000000',
  ],
  '4': [
    '00001100',
    '00011100',
    '00101100',
    '01001100',
    '01111110',
    '00001100',
    '00001100',
    '00000000',
  ],
  '5': [
    '01111110',
    '01100000',
    '01111100',
    '00000110',
    '00000110',
    '01100110',
    '00111100',
    '00000000',
  ],
  '6': [
    '00111100',
    '01100110',
    '01100000',
    '01111100',
    '01100110',
    '01100110',
    '00111100',
    '00000000',
  ],
  '7': [
    '01111110',
    '00000110',
    '00001100',
    '00011000',
    '00110000',
    '00110000',
    '00110000',
    '00000000',
  ],
  '8': [
    '00111100',
    '01100110',
    '01100110',
    '00111100',
    '01100110',
    '01100110',
    '00111100',
    '00000000',
  ],
  '9': [
    '00111100',
    '01100110',
    '01100110',
    '00111110',
    '00000110',
    '01100110',
    '00111100',
    '00000000',
  ],
  '+': [
    '00000000',
    '00011000',
    '00011000',
    '01111110',
    '00011000',
    '00011000',
    '00000000',
    '00000000',
  ],
  '-': [
    '00000000',
    '00000000',
    '00000000',
    '01111110',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
  ],
  '.': [
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00011000',
    '00011000',
    '00000000',
  ],
  'i': [
    '00011000',
    '00000000',
    '00011000',
    '00011000',
    '00011000',
    '00011000',
    '00110000',
    '00000000',
  ],
  'q': [
    '00000000',
    '00111000',
    '01000100',
    '01000100',
    '01000100',
    '00111000',
    '00000100',
    '00000110',
  ],
  ':': [
    '00000000',
    '00011000',
    '00011000',
    '00000000',
    '00011000',
    '00011000',
    '00000000',
    '00000000',
  ],
  '(': [
    '00000110',
    '00001100',
    '00011000',
    '00011000',
    '00011000',
    '00001100',
    '00000110',
    '00000000',
  ],
  ')': [
    '01100000',
    '00110000',
    '00011000',
    '00011000',
    '00011000',
    '00110000',
    '01100000',
    '00000000',
  ],
  '[': [
    '00111100',
    '00110000',
    '00110000',
    '00110000',
    '00110000',
    '00110000',
    '00111100',
    '00000000',
  ],
  ']': [
    '00111100',
    '00001100',
    '00001100',
    '00001100',
    '00001100',
    '00001100',
    '00111100',
    '00000000',
  ],
  ',': [
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00011000',
    '00011000',
    '00110000',
  ],
  ' ': [
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
    '00000000',
  ],
}

function createBlankGlyph(size: number): number[][] {
  return Array.from({ length: size }, () => Array.from({ length: size }, () => 0))
}

function drawRect(grid: number[][], x: number, y: number, w: number, h: number) {
  for (let row = y; row < y + h; row += 1) {
    for (let col = x; col < x + w; col += 1) {
      if (row >= 0 && row < grid.length && col >= 0 && col < grid.length) {
        grid[row][col] = 1
      }
    }
  }
}

function distanceToSegment(px: number, py: number, x0: number, y0: number, x1: number, y1: number) {
  const dx = x1 - x0
  const dy = y1 - y0
  const lenSq = dx * dx + dy * dy
  if (lenSq === 0) {
    return Math.hypot(px - x0, py - y0)
  }
  const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / lenSq))
  const projX = x0 + t * dx
  const projY = y0 + t * dy
  return Math.hypot(px - projX, py - projY)
}

function drawLine(grid: number[][], x0: number, y0: number, x1: number, y1: number, thickness: number) {
  const half = thickness / 2
  for (let y = 0; y < grid.length; y += 1) {
    for (let x = 0; x < grid.length; x += 1) {
      const dist = distanceToSegment(x + 0.5, y + 0.5, x0, y0, x1, y1)
      if (dist <= half) {
        grid[y][x] = 1
      }
    }
  }
}

function glyphToRows(grid: number[][]): string[] {
  return grid.map((row) => row.map((cell) => (cell ? '1' : '0')).join(''))
}

export function buildLabelGlyphs(size: number): GlyphMap {
  const stroke = Math.max(2, Math.round(size / 9))
  const inset = Math.max(2, Math.round(size / 8))
  const mid = Math.floor(size / 2)

  const hGrid = createBlankGlyph(size)
  drawRect(hGrid, inset, inset, stroke, size - inset * 2)
  drawRect(hGrid, size - inset - stroke, inset, stroke, size - inset * 2)
  drawRect(hGrid, inset, mid - Math.floor(stroke / 2), size - inset * 2, stroke)

  const xGrid = createBlankGlyph(size)
  drawLine(xGrid, inset, inset, size - inset, size - inset, stroke)
  drawLine(xGrid, inset, size - inset, size - inset, inset, stroke)

  const yGrid = createBlankGlyph(size)
  drawLine(yGrid, inset, inset, mid, mid, stroke)
  drawLine(yGrid, size - inset, inset, mid, mid, stroke)
  drawRect(yGrid, mid - Math.floor(stroke / 2), mid, stroke, size - inset - mid)

  const zGrid = createBlankGlyph(size)
  drawRect(zGrid, inset, inset, size - inset * 2, stroke)
  drawRect(zGrid, inset, size - inset - stroke, size - inset * 2, stroke)
  drawLine(zGrid, size - inset, inset + stroke, inset, size - inset - stroke, stroke)

  const sGrid = createBlankGlyph(size)
  drawRect(sGrid, inset, inset, size - inset * 2, stroke)
  drawRect(sGrid, inset, mid - Math.floor(stroke / 2), size - inset * 2, stroke)
  drawRect(sGrid, inset, size - inset - stroke, size - inset * 2, stroke)
  drawRect(sGrid, inset, inset, stroke, mid - inset)
  drawRect(sGrid, size - inset - stroke, mid, stroke, size - inset - mid)

  const tGrid = createBlankGlyph(size)
  drawRect(tGrid, inset, inset, size - inset * 2, stroke)
  drawRect(tGrid, mid - Math.floor(stroke / 2), inset, stroke, size - inset * 2)

  const sqrtGrid = createBlankGlyph(size)
  drawLine(sqrtGrid, inset, mid, mid - Math.floor(stroke / 2), size - inset, stroke)
  drawLine(sqrtGrid, mid - Math.floor(stroke / 2), size - inset, size - inset, inset, stroke)

  const daggerGrid = createBlankGlyph(size)
  drawRect(daggerGrid, mid - Math.floor(stroke / 2), inset, stroke, size - inset * 2)
  drawRect(daggerGrid, inset, mid - Math.floor(stroke / 2), size - inset * 2, stroke)

  const spaceGrid = createBlankGlyph(size)

  return {
    H: glyphToRows(hGrid),
    X: glyphToRows(xGrid),
    Y: glyphToRows(yGrid),
    Z: glyphToRows(zGrid),
    S: glyphToRows(sGrid),
    T: glyphToRows(tGrid),
    '^': glyphToRows(sqrtGrid),
    '|': glyphToRows(daggerGrid),
    ' ': glyphToRows(spaceGrid),
  }
}

export function createFontAtlas(glyphSize: number, glyphs: GlyphMap) {
  const atlasWidth = FONT_COLS * glyphSize
  const atlasHeight = FONT_ROWS * glyphSize
  const data = new Uint8Array(atlasWidth * atlasHeight * 4)

  const setGlyph = (char: string, rows: string[]) => {
    const code = char.charCodeAt(0)
    const index = code - 32
    if (index < 0 || index >= FONT_COLS * FONT_ROWS) {
      return
    }
    const col = index % FONT_COLS
    const row = Math.floor(index / FONT_COLS)
    const baseX = col * glyphSize
    const baseY = row * glyphSize
    rows.forEach((rowBits, y) => {
      for (let x = 0; x < glyphSize; x += 1) {
        const value = rowBits[x] === '1' ? 255 : 0
        const px = baseX + x
        const py = baseY + y
        const offset = (py * atlasWidth + px) * 4
        data[offset] = 255
        data[offset + 1] = 255
        data[offset + 2] = 255
        data[offset + 3] = value
      }
    })
  }

  Object.entries(glyphs).forEach(([char, rows]) => setGlyph(char, rows))

  return { data, atlasWidth, atlasHeight }
}

export { LABEL_BASE_GLYPHS }
