const test = require('node:test')
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const root = path.resolve(__dirname, '..')
const appSource = fs.readFileSync(path.join(root, 'src/app.rs'), 'utf8')
const popupAtlasSource = fs.readFileSync(path.join(root, 'src/gpu/popup_glyph_atlas.rs'), 'utf8')
const geistMonoPath = path.join(root, 'assets/GeistMono-Regular.ttf')
const geistMonoBytes = fs.readFileSync(geistMonoPath)

type TableMap = Map<string, { offset: number; length: number }>

const readU16 = (bytes: Buffer, offset: number): number => bytes.readUInt16BE(offset)
const readI16 = (bytes: Buffer, offset: number): number => bytes.readInt16BE(offset)
const readU32 = (bytes: Buffer, offset: number): number => bytes.readUInt32BE(offset)

const readTables = (bytes: Buffer): TableMap => {
  const numTables = readU16(bytes, 4)
  const tables: TableMap = new Map()
  for (let index = 0; index < numTables; index += 1) {
    const record = 12 + index * 16
    const tag = bytes.toString('ascii', record, record + 4)
    tables.set(tag, {
      offset: readU32(bytes, record + 8),
      length: readU32(bytes, record + 12),
    })
  }
  return tables
}

const format4HasCodePoint = (bytes: Buffer, table: number, codePoint: number): boolean => {
  const segCount = readU16(bytes, table + 6) / 2
  const endCode = table + 14
  const startCode = endCode + segCount * 2 + 2
  const idDelta = startCode + segCount * 2
  const idRangeOffset = idDelta + segCount * 2
  for (let segment = 0; segment < segCount; segment += 1) {
    const end = readU16(bytes, endCode + segment * 2)
    if (codePoint > end) {
      continue
    }
    const start = readU16(bytes, startCode + segment * 2)
    if (codePoint < start) {
      return false
    }
    const rangeOffsetPos = idRangeOffset + segment * 2
    const rangeOffset = readU16(bytes, rangeOffsetPos)
    if (rangeOffset === 0) {
      return ((codePoint + readI16(bytes, idDelta + segment * 2)) & 0xffff) !== 0
    }
    const glyphOffset = rangeOffsetPos + rangeOffset + (codePoint - start) * 2
    return glyphOffset + 2 <= bytes.length && readU16(bytes, glyphOffset) !== 0
  }
  return false
}

const format12HasCodePoint = (bytes: Buffer, table: number, codePoint: number): boolean => {
  const groups = readU32(bytes, table + 12)
  for (let index = 0; index < groups; index += 1) {
    const group = table + 16 + index * 12
    const start = readU32(bytes, group)
    const end = readU32(bytes, group + 4)
    if (codePoint >= start && codePoint <= end) {
      return readU32(bytes, group + 8) + codePoint - start !== 0
    }
  }
  return false
}

const fontHasCodePoint = (bytes: Buffer, codePoint: number): boolean => {
  const cmap = readTables(bytes).get('cmap')
  if (!cmap) {
    throw new Error('font must contain cmap table')
  }
  const tableStart = cmap.offset
  const subtables = readU16(bytes, tableStart + 2)
  for (let index = 0; index < subtables; index += 1) {
    const record = tableStart + 4 + index * 8
    const subtable = tableStart + readU32(bytes, record + 4)
    const format = readU16(bytes, subtable)
    if (format === 4 && codePoint <= 0xffff && format4HasCodePoint(bytes, subtable, codePoint)) {
      return true
    }
    if (format === 12 && format12HasCodePoint(bytes, subtable, codePoint)) {
      return true
    }
  }
  return false
}

test('egui font setup makes Geist the primary UI voice with Hack as fallback', () => {
  assert.match(appSource, /FontDefinitions::empty\(\)/)
  assert.match(appSource, /"geist_regular"\.to_owned\(\),\s*"hack_fallback"\.to_owned\(\)/s)
  assert.match(appSource, /"geist_mono"\.to_owned\(\),\s*"hack_fallback"\.to_owned\(\)/s)
  assert.doesNotMatch(appSource, /for family in \[egui::FontFamily::Proportional, egui::FontFamily::Monospace\]/)
})

test('Geist Mono asset is checked in and used for the popup glyph atlas', () => {
  assert.ok(geistMonoBytes.length > 100_000)
  assert.deepEqual([...geistMonoBytes.subarray(0, 4)], [0, 1, 0, 0])
  assert.match(popupAtlasSource, /GeistMono-Regular\.ttf/)
  assert.doesNotMatch(popupAtlasSource, /HACK_REGULAR/)
})

test('Geist Mono covers every shader-formatted popup value glyph', () => {
  for (const glyph of ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '.', '+', '-', 'i', '%', '°']) {
    assert.equal(fontHasCodePoint(geistMonoBytes, glyph.codePointAt(0)!), true, `missing ${glyph}`)
  }
})

export {}
