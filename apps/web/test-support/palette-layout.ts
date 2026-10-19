import { UI_CONSTANTS } from './generated-ui-constants'

// Mirrors the full palette layout in `src/layout/palette.rs`.

type Point = { x: number; y: number }

type FullPaletteGrid = {
  row1Columns: number
  row2Columns: number
  displayColumns: number
  displayRows: number
  displayX: number
  totalWidth: number
  rows: number
}

const ROW1_INDICES = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 36, 37]
const ROW1_COUNT = ROW1_INDICES.length
const ROW2_INDICES = [13, 14, 15, 31, 32, 17, 18, 19, 26, 34, 35, 22, 23, 27, 28, 29, 30]
// 表示と、見た目のための指示（注釈や空きマス）。
const DISPLAY_INDICES = [16, 20, 33, 24, 25]
const INSTRUCTION_INDICES = [21, 38]
const PANEL_MARGIN = 8
const DISPLAY_GRID_CHOICES = [
  { columns: UI_CONSTANTS.PALETTE_DISPLAY_COLUMNS, rows: UI_CONSTANTS.PALETTE_DISPLAY_ROWS },
  { columns: 3, rows: 3 },
  { columns: 2, rows: 4 },
]
const PITCH_X = UI_CONSTANTS.PALETTE_SIZE + UI_CONSTANTS.PALETTE_GAP
const PITCH_Y = UI_CONSTANTS.PALETTE_SIZE + UI_CONSTANTS.PALETTE_ROW_GAP

const rowWidth = (count: number): number => (count > 0 ? count * PITCH_X - UI_CONSTANTS.PALETTE_GAP : 0)
const columnHeight = (rows: number): number => (rows > 0 ? rows * PITCH_Y - UI_CONSTANTS.PALETTE_ROW_GAP : 0)
const balancedColumns = (count: number, limit: number): number => Math.ceil(count / Math.ceil(count / limit))
const rowCounts = (count: number, columns: number): number[] =>
  Array.from({ length: Math.ceil(count / columns) }, (_, row) => Math.min(count - row * columns, columns))

const fullPaletteGrid = (limit: number, displayColumns: number, displayRows: number): FullPaletteGrid => {
  const row1Columns = balancedColumns(ROW1_COUNT, limit)
  const row2Columns = balancedColumns(ROW2_INDICES.length, limit)
  const gateRows = [...rowCounts(ROW1_COUNT, row1Columns), ...rowCounts(ROW2_INDICES.length, row2Columns)]
  const besideWidth = Math.max(0, ...gateRows.slice(0, displayRows).map(rowWidth))
  const displayX = besideWidth + 2 * UI_CONSTANTS.PALETTE_SECTION_GAP + UI_CONSTANTS.PALETTE_SEPARATOR_WIDTH
  return {
    row1Columns,
    row2Columns,
    displayColumns,
    displayRows,
    displayX,
    totalWidth: Math.max(displayX + rowWidth(displayColumns), ...gateRows.map(rowWidth)),
    rows: Math.max(gateRows.length, displayRows),
  }
}

const fullPaletteGridFor = (panelWidth: number): FullPaletteGrid => {
  const available = panelWidth - 2 * (UI_CONSTANTS.PALETTE_MARGIN_X + UI_CONSTANTS.PALETTE_PADDING_X)
  let best: FullPaletteGrid | undefined
  for (const { columns, rows } of DISPLAY_GRID_CHOICES) {
    for (let limit = ROW2_INDICES.length; limit >= 1; limit -= 1) {
      const grid = fullPaletteGrid(limit, columns, rows)
      if (grid.totalWidth <= available && (!best || grid.rows < best.rows)) {
        best = grid
      }
    }
  }
  return best ?? fullPaletteGrid(1, DISPLAY_GRID_CHOICES[2].columns, DISPLAY_GRID_CHOICES[2].rows)
}

// Spacer and the comment block follow the displays, moving to the next row
// together when they would otherwise be split across rows.
const instructionStartSlot = (columns: number): number => {
  const displays = DISPLAY_INDICES.length
  return displays % columns + INSTRUCTION_INDICES.length > columns
    ? Math.ceil(displays / columns) * columns
    : displays
}

const localPos = (grid: FullPaletteGrid, gateIndex: number): Point => {
  const row1Pos = ROW1_INDICES.indexOf(gateIndex)
  if (row1Pos >= 0) {
    return { x: (row1Pos % grid.row1Columns) * PITCH_X, y: Math.floor(row1Pos / grid.row1Columns) * PITCH_Y }
  }
  const row2Pos = ROW2_INDICES.indexOf(gateIndex)
  if (row2Pos >= 0) {
    const row = Math.ceil(ROW1_COUNT / grid.row1Columns) + Math.floor(row2Pos / grid.row2Columns)
    return { x: (row2Pos % grid.row2Columns) * PITCH_X, y: row * PITCH_Y }
  }
  const instructionPos = INSTRUCTION_INDICES.indexOf(gateIndex)
  const slot = instructionPos >= 0
    ? instructionStartSlot(grid.displayColumns) + instructionPos
    : DISPLAY_INDICES.indexOf(gateIndex)
  if (slot >= 0) {
    return {
      x: grid.displayX + (slot % grid.displayColumns) * PITCH_X,
      y: Math.floor(slot / grid.displayColumns) * PITCH_Y,
    }
  }
  throw new Error(`unknown palette gate index: ${gateIndex}`)
}

/** Canvas position of a full-palette gate's centre on a `cssWidth` px canvas. */
export const getPaletteGateCenter = (cssWidth: number, gateIndex: number): Point => {
  const panelWidth = cssWidth - 2 * PANEL_MARGIN
  const grid = fullPaletteGridFor(panelWidth)
  const offset = (panelWidth - grid.totalWidth) / 2
  const startX = PANEL_MARGIN + Math.sign(offset) * Math.round(Math.abs(offset))
  const local = localPos(grid, gateIndex)
  return {
    x: startX + local.x + UI_CONSTANTS.PALETTE_SIZE / 2,
    y: UI_CONSTANTS.PALETTE_ROW_Y + local.y + UI_CONSTANTS.PALETTE_SIZE / 2,
  }
}

/**
 * How far the full palette moves the circuit down from `LINE_Y` on a
 * `cssWidth` px canvas: `0` while it fits in two rows.
 */
export const fullPaletteCircuitOffsetY = (cssWidth: number): number =>
  columnHeight(fullPaletteGridFor(cssWidth - 2 * PANEL_MARGIN).rows) - columnHeight(UI_CONSTANTS.PALETTE_DISPLAY_ROWS)
