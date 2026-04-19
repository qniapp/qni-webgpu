const { test, expect } = require('@playwright/test')
const { chromium } = require('playwright')

const evaluateWithRetry = async (page, fn, arg, attempts = 3) => {
  let lastError
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await page.evaluate(fn, arg)
    } catch (error) {
      lastError = error
      if (!String(error).includes('Execution context was destroyed')) {
        throw error
      }
      await page.waitForLoadState('load')
      await page.waitForFunction(
        () => window.__eguiReady === true || Boolean(window.__eguiError),
        null,
        { timeout: 20000 }
      )
    }
  }
  throw lastError
}

const screenshotWithRetry = async (page, locator, options, attempts = 3) => {
  let lastError
  for (let i = 0; i < attempts; i += 1) {
    try {
      return await locator.screenshot(options)
    } catch (error) {
      lastError = error
      const message = String(error)
      if (
        !message.includes('Execution context was destroyed') &&
        !message.includes('Element is not attached') &&
        !message.includes('Cannot find context')
      ) {
        throw error
      }
      await page.waitForLoadState('load')
      await page.waitForFunction(
        () => window.__eguiReady === true || Boolean(window.__eguiError),
        null,
        { timeout: 20000 }
      )
      await waitForStateVectorReady(page)
    }
  }
  throw lastError
}

const sampleCanvasPixels = async (page, locator, samples) => {
  const screenshot = await screenshotWithRetry(page, locator, { type: 'png' })
  const base64 = screenshot.toString('base64')
  const box = await locator.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  return evaluateWithRetry(
    page,
    async ({ base64, samples, cssWidth, cssHeight }) => {
      const img = new Image()
      img.src = `data:image/png;base64,${base64}`
      await new Promise((resolve, reject) => {
        img.onload = () => resolve(null)
        img.onerror = () => reject(new Error('Failed to decode screenshot'))
      })

      const canvas = document.createElement('canvas')
      canvas.width = img.width
      canvas.height = img.height
      const ctx = canvas.getContext('2d', { willReadFrequently: true })
      if (!ctx) {
        return {}
      }
      ctx.drawImage(img, 0, 0)

      const scaleX = img.width / cssWidth
      const scaleY = img.height / cssHeight

      return Object.fromEntries(
        samples.map(({ name, x, y }) => {
          const data = ctx.getImageData(
            Math.floor(x * scaleX),
            Math.floor(y * scaleY),
            1,
            1
          ).data
          return [name, Array.from(data)]
        })
      )
    },
    { base64, samples, cssWidth: box.width, cssHeight: box.height }
  )
}

const readStateVector = async (page) =>
  evaluateWithRetry(page, async () => {
    if (!window.__eguiReadStateVector) {
      return []
    }
    return await window.__eguiReadStateVector()
  })

const waitForStateVectorReady = async (page, timeout = 20000) => {
  await expect
    .poll(async () => (await readStateVector(page)).length > 0, { timeout })
    .toBe(true)
}

const waitForStateVectorLength = async (page, length, timeout = 5000) => {
  await expect
    .poll(async () => (await readStateVector(page)).length, { timeout })
    .toBe(length)
}

const waitForStateVectorApprox = async (page, expected, timeout = 5000, tolerance = 1e-3) => {
  await expect
    .poll(async () => {
      const actual = await readStateVector(page)
      if (actual.length !== expected.length) {
        return false
      }
      return expected.every((value, index) => Math.abs(actual[index] - value) < tolerance)
    }, { timeout })
    .toBe(true)
}

const waitForCanvasBlue = async (page, canvas, path, timeout = 5000) => {
  const start = Date.now()
  let lastHits = 0
  let lastScreenshot
  while (Date.now() - start < timeout) {
    lastScreenshot = await screenshotWithRetry(page, canvas, { type: 'png', path })
    const base64 = lastScreenshot.toString('base64')
    const { hits } = await evaluateWithRetry(
      page,
      async ({ base64 }) => {
        const img = new Image()
        img.src = `data:image/png;base64,${base64}`
        await new Promise((resolve, reject) => {
          img.onload = () => resolve(null)
          img.onerror = () => reject(new Error('Failed to decode screenshot'))
        })

        const canvas = document.createElement('canvas')
        canvas.width = img.width
        canvas.height = img.height
        const ctx = canvas.getContext('2d', { willReadFrequently: true })
        if (!ctx) {
          return { hits: 0 }
        }
        ctx.drawImage(img, 0, 0)
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
        const data = imageData.data

        let hits = 0
        for (let y = Math.floor(canvas.height * 0.6); y < canvas.height; y += 2) {
          for (let x = 0; x < canvas.width; x += 2) {
            const idx = (y * canvas.width + x) * 4
            const r = data[idx]
            const g = data[idx + 1]
            const b = data[idx + 2]
            if (b > r + 30 && b > g + 30) {
              hits += 1
            }
          }
        }
        return { hits }
      },
      { base64 }
    )
    lastHits = hits
    if (hits > 100) {
      return { hits, screenshot: lastScreenshot }
    }
    await page.waitForTimeout(100)
  }
  return { hits: lastHits, screenshot: lastScreenshot }
}

const dragPointer = async (page, from, to, steps = 6, release = true) => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  const startX = box.x + from.x
  const startY = box.y + from.y
  const endX = box.x + to.x
  const endY = box.y + to.y
  await page.mouse.move(startX, startY)
  await page.mouse.down()
  await page.waitForTimeout(16)
  await page.mouse.move(endX, endY, { steps })
  await page.waitForTimeout(16)
  if (release) {
    await page.mouse.up()
  }
}

const releasePointer = async (page, at) => {
  const canvas = page.locator('#egui-canvas')
  const box = await canvas.boundingBox()
  if (!box) {
    throw new Error('canvas not found')
  }
  const endX = box.x + at.x
  const endY = box.y + at.y
  await page.mouse.move(endX, endY)
  await page.mouse.up()
}

test('egui webgpu canvas renders content', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  const gpuAvailable = await page.evaluate(() => Boolean(navigator.gpu))
  expect(gpuAvailable).toBe(true)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)
  const cssHeight = box?.height ?? (viewport?.height ?? 800)

  await waitForStateVectorReady(page)
  const initialState = await readStateVector(page)
  expect(initialState).toEqual([1, 0, 0, 0])
  const stateCount = Math.max(1, initialState.length / 2)

  await page.waitForTimeout(500)
  const { hits: initialHits } = await waitForCanvasBlue(
    page,
    canvas,
    '/tmp/qni-egui-webgpu-initial.png'
  )
  expect(initialHits).toBeGreaterThan(100)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2
  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y
  const targetX2 = targetX + SLOT_SPACING
  const targetY2 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY })

  const expected = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY })

  await waitForStateVectorApprox(page, expected)

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX2, y: targetY2 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)

  const screenshot = await screenshotWithRetry(page, canvas, {
    type: 'png',
    path: '/tmp/qni-egui-webgpu-after.png'
  })
  const base64 = screenshot.toString('base64')

  const stats = await evaluateWithRetry(page, async ({ base64 }) => {
    const img = new Image()
    img.src = `data:image/png;base64,${base64}`
    await new Promise((resolve, reject) => {
      img.onload = () => resolve(null)
      img.onerror = () => reject(new Error('Failed to decode screenshot'))
    })

    const canvas = document.createElement('canvas')
    canvas.width = img.width
    canvas.height = img.height
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) {
      return { nonBackground: 0 }
    }
    ctx.drawImage(img, 0, 0)
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height)
    const data = imageData.data
    const width = imageData.width
    const height = imageData.height
    const background = [255, 255, 255]
    const threshold = 20
    let nonBackground = 0

    for (let y = 0; y < height; y += 4) {
      for (let x = 0; x < width; x += 4) {
        const idx = (y * width + x) * 4
        const diff =
          Math.abs(data[idx] - background[0]) +
          Math.abs(data[idx + 1] - background[1]) +
          Math.abs(data[idx + 2] - background[2])
        if (diff > threshold) {
          nonBackground += 1
        }
      }
    }

    return { nonBackground }
  }, { base64 })

  expect(stats.nonBackground).toBeGreaterThan(40)
  await canvas.screenshot({ path: '/tmp/qni-egui-webgpu.png' })
})

test('H on q0 and q1 yields uniform superposition', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)
  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  const expectedAfterQ0 = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedAfterQ0)

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })

  const expected = [0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0]
  await waitForStateVectorApprox(page, expected)
})

test('dragging does not grow state vector until drop', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP
  const targetY2 = LINE_Y + 2 * LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })
  await waitForStateVectorLength(page, 8)

  await dragPointer(page, { x: targetX, y: targetY0 }, { x: targetX, y: targetY2 }, 6, false)

  const lengthDuringDrag = (await readStateVector(page)).length
  expect(lengthDuringDrag).toBe(8)

  await releasePointer(page, { x: targetX, y: targetY2 })

  await waitForStateVectorLength(page, 16)
})

test('palette panel keeps its corners and shadow while dragging', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const PALETTE_SIZE = REM
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const palettePadding = REM
  const paletteRect = {
    x: paletteStartX - palettePadding,
    y: PALETTE_ROW_Y - palettePadding,
    width: paletteWidth + palettePadding * 2,
    height: PALETTE_SIZE + palettePadding * 2,
  }
  const sourceX = paletteStartX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const dragTarget = { x: sourceX + 80, y: sourceY + 80 }
  const panelPoints = [
    { name: 'corner', x: paletteRect.x + 2, y: paletteRect.y + 2 },
    { name: 'fill', x: paletteRect.x + 24, y: paletteRect.y + 24 },
    { name: 'shadow', x: paletteRect.x + paletteRect.width / 2, y: paletteRect.y + paletteRect.height + 10 },
    { name: 'background', x: paletteRect.x - 20, y: paletteRect.y + paletteRect.height + 10 },
  ]

  const beforeDrag = await sampleCanvasPixels(page, canvas, panelPoints)

  await dragPointer(page, { x: sourceX, y: sourceY }, dragTarget, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, panelPoints)

  for (const name of ['corner', 'fill']) {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    expect(diff).toBeLessThan(40)
  }

  const cornerBrightness = duringDrag.corner[0] + duringDrag.corner[1] + duringDrag.corner[2]
  const fillBrightness = duringDrag.fill[0] + duringDrag.fill[1] + duringDrag.fill[2]
  expect(Math.abs(cornerBrightness - fillBrightness)).toBeGreaterThan(10)

  const shadowBrightness = duringDrag.shadow[0] + duringDrag.shadow[1] + duringDrag.shadow[2]
  const backgroundBrightness = duringDrag.background[0] + duringDrag.background[1] + duringDrag.background[2]
  expect(Math.abs(shadowBrightness - backgroundBrightness)).toBeGreaterThan(10)

  await page.mouse.up()
})

test('palette control gate keeps its icon while dragging', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const PALETTE_SIZE = REM
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const dragSource = { x: paletteStartX + PALETTE_SIZE / 2, y: PALETTE_ROW_Y + PALETTE_SIZE / 2 }
  const dragTarget = { x: dragSource.x + 80, y: dragSource.y + 80 }
  const controlIndex = 1
  const controlRect = {
    x: paletteStartX + controlIndex * (PALETTE_SIZE + PALETTE_GAP),
    y: PALETTE_ROW_Y,
  }
  const signaturePoints = [
    { name: 'center', x: controlRect.x + PALETTE_SIZE / 2, y: controlRect.y + PALETTE_SIZE / 2 },
    { name: 'top', x: controlRect.x + PALETTE_SIZE / 2, y: controlRect.y + 6 },
    { name: 'bottom', x: controlRect.x + PALETTE_SIZE / 2, y: controlRect.y + PALETTE_SIZE - 6 },
    { name: 'left', x: controlRect.x + 6, y: controlRect.y + PALETTE_SIZE / 2 },
    { name: 'right', x: controlRect.x + PALETTE_SIZE - 6, y: controlRect.y + PALETTE_SIZE / 2 },
  ]

  const beforeDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await dragPointer(page, dragSource, dragTarget, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  for (const name of Object.keys(beforeDrag)) {
    const before = beforeDrag[name]
    const during = duringDrag[name]
    const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
    expect(diff).toBeLessThan(40)
  }

  await page.mouse.up()
})

test('dragged palette gate stays visible above the palette panel', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const sourceX = paletteStartX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const dragTarget = {
    x: paletteStartX - PALETTE_SIZE / 2,
    y: sourceY,
  }
  const dragRect = {
    x: dragTarget.x - GATE_SIZE / 2,
    y: dragTarget.y - GATE_SIZE / 2,
  }
  const fillPoint = {
    name: 'fill',
    x: dragRect.x + GATE_SIZE - 6,
    y: dragRect.y + GATE_SIZE - 6,
  }

  const beforeDrag = await sampleCanvasPixels(page, canvas, [fillPoint])

  await dragPointer(page, { x: sourceX, y: sourceY }, dragTarget, 6, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [fillPoint])

  const before = beforeDrag.fill
  const during = duringDrag.fill
  const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
  expect(diff).toBeGreaterThan(120)
  expect(during[1]).toBeGreaterThan(during[0] + 40)

  await page.mouse.up()
})

test('dragged palette gate stays above the state panel overlay', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)
  const cssHeight = box?.height ?? (viewport?.height ?? 700)

  const REM = 32
  const STATE_CIRCLE_SIZE = 1.25 * REM
  const STATE_CIRCLE_GAP = 0.5 * REM
  const STATE_CIRCLE_BOTTOM_MARGIN = 2 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_SIZE = 1 * REM
  const stateCount = 4
  const statePadding = Math.min(REM, cssWidth * 0.05, cssHeight * 0.05)
  const topLimit = PALETTE_ROW_Y + PALETTE_SIZE + 2 * REM
  let availableWidth = cssWidth - statePadding * 2
  let availableHeight = cssHeight - STATE_CIRCLE_BOTTOM_MARGIN - topLimit
  if (availableWidth <= 0) {
    availableWidth = Math.max(cssWidth, 1)
  }
  if (availableHeight <= 0) {
    availableHeight = Math.max(cssHeight - STATE_CIRCLE_BOTTOM_MARGIN, 1)
  }
  const maxHeight = cssHeight * 0.4
  if (availableHeight > maxHeight) {
    availableHeight = Math.max(maxHeight, 1)
  }

  const gapRatio = STATE_CIRCLE_GAP / STATE_CIRCLE_SIZE
  let columns = 1
  let rows = stateCount
  let bestSize = 0
  let bestScore = Number.POSITIVE_INFINITY
  const divisors = [1, 2, 4]
  for (const candidate of divisors) {
    if (stateCount % candidate !== 0) {
      continue
    }
    const candidateRows = stateCount / candidate
    const sizeW = availableWidth / (candidate + (candidate - 1) * gapRatio)
    const sizeH = availableHeight / (candidateRows + (candidateRows - 1) * gapRatio)
    const size = Math.min(sizeW, sizeH, STATE_CIRCLE_SIZE)
    const ratio = candidate / candidateRows
    const score = Math.abs(ratio - Math.max(availableWidth / availableHeight, 0.1))
    if (size > bestSize + 0.01 || (Math.abs(size - bestSize) <= 0.01 && score < bestScore)) {
      columns = candidate
      rows = candidateRows
      bestSize = size
      bestScore = score
    }
  }
  const size = Math.max(bestSize, 0.5)
  const gap = size * gapRatio
  const totalWidth = size * columns + gap * Math.max(columns - 1, 0)
  const totalHeight = size * rows + gap * Math.max(rows - 1, 0)
  const baseX = cssWidth / 2 - totalWidth / 2
  const baseY = cssHeight - STATE_CIRCLE_BOTTOM_MARGIN - totalHeight
  const contentHeight = totalHeight + statePadding * 2
  const handleHeight = Math.max(Math.min(0.4 * REM, contentHeight * 0.4), 10)
  const handlePadding = handleHeight * 0.5
  const stateRectMin = {
    x: baseX - statePadding,
    y: baseY - (statePadding + handleHeight + handlePadding),
  }
  const handleCenter = {
    x: baseX + totalWidth / 2,
    y: stateRectMin.y + handleHeight / 2,
  }

  const PALETTE_GAP = 0.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const paletteStartX = cssWidth / 2 - paletteWidth / 2
  const source = {
    x: paletteStartX + PALETTE_SIZE / 2,
    y: PALETTE_ROW_Y + PALETTE_SIZE / 2,
  }
  const dragFillPoint = {
    name: 'fill',
    x: handleCenter.x + PALETTE_SIZE / 2 - 6,
    y: handleCenter.y + PALETTE_SIZE / 2 - 6,
  }
  const beforeDrag = await sampleCanvasPixels(page, canvas, [dragFillPoint])

  await dragPointer(page, source, handleCenter, 8, false)
  await page.waitForTimeout(50)

  const duringDrag = await sampleCanvasPixels(page, canvas, [dragFillPoint])

  const before = beforeDrag.fill
  const during = duringDrag.fill
  const diff = Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
  expect(diff).toBeGreaterThan(120)
  expect(during[1]).toBeGreaterThan(during[0] + 40)

  await page.mouse.up()
})

test('dragged palette gate keeps rounded corners', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const sourceX = startX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const dragTarget = { x: sourceX + 80, y: sourceY + 80 }
  const dragRect = {
    x: dragTarget.x - GATE_SIZE / 2,
    y: dragTarget.y - GATE_SIZE / 2,
  }

  await dragPointer(page, { x: sourceX, y: sourceY }, dragTarget, 6, false)
  await page.waitForTimeout(50)

  const pixels = await sampleCanvasPixels(page, canvas, [
    { name: 'corner', x: dragRect.x + 1, y: dragRect.y + 1 },
    { name: 'fill', x: dragRect.x + GATE_SIZE - 6, y: dragRect.y + GATE_SIZE - 6 },
  ])

  const cornerBrightness = pixels.corner[0] + pixels.corner[1] + pixels.corner[2]
  const fillBrightness = pixels.fill[0] + pixels.fill[1] + pixels.fill[2]
  expect(cornerBrightness).toBeGreaterThan(fillBrightness + 100)

  await page.mouse.up()
})

test('dragged x gate keeps the same visual as after drop', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const gateIndex = 2
  const sourceX = startX + gateIndex * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const targetRect = {
    x: targetCenter.x - GATE_SIZE / 2,
    y: targetCenter.y - GATE_SIZE / 2,
  }
  const signaturePoints = [
    { name: 'center', x: targetRect.x + GATE_SIZE / 2, y: targetRect.y + GATE_SIZE / 2 },
    { name: 'top', x: targetRect.x + GATE_SIZE / 2, y: targetRect.y + 6 },
    { name: 'bottom', x: targetRect.x + GATE_SIZE / 2, y: targetRect.y + GATE_SIZE - 6 },
    { name: 'left', x: targetRect.x + 6, y: targetRect.y + GATE_SIZE / 2 },
    { name: 'right', x: targetRect.x + GATE_SIZE - 6, y: targetRect.y + GATE_SIZE / 2 },
  ]

  await dragPointer(page, { x: sourceX, y: sourceY }, targetCenter, 6, false)
  await page.waitForTimeout(50)
  const duringDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await releasePointer(page, targetCenter)
  await page.waitForTimeout(50)
  const afterDrop = await sampleCanvasPixels(page, canvas, signaturePoints)

  for (const name of Object.keys(duringDrag)) {
    const during = duringDrag[name]
    const after = afterDrop[name]
    const diff = Math.abs(during[0] - after[0]) + Math.abs(during[1] - after[1]) + Math.abs(during[2] - after[2])
    expect(diff).toBeLessThan(60)
  }
})

test('placed circuit gate keeps its visual while dragging another gate', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const sqrtXGateCenter = { x: paletteCenterX(5), y: sourceY }
  const hGateCenter = { x: paletteCenterX(0), y: sourceY }
  const placedGateCenter = { x: LINE_LEFT_OFFSET + GATE_SIZE, y: LINE_Y }
  const placedGateRect = {
    x: placedGateCenter.x - GATE_SIZE / 2,
    y: placedGateCenter.y - GATE_SIZE / 2,
  }

  const signaturePoints = []
  let pointIndex = 0
  for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 5; col++) {
      signaturePoints.push({
        name: `p${pointIndex++}`,
        x: placedGateRect.x + 5 + col * 5,
        y: placedGateRect.y + 5 + row * 5,
      })
    }
  }

  await dragPointer(page, sqrtXGateCenter, placedGateCenter)
  await page.waitForTimeout(50)
  await page.mouse.move((box?.x ?? 0) + placedGateCenter.x + 120, (box?.y ?? 0) + placedGateCenter.y + 120)
  await page.waitForTimeout(50)
  const beforeDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  await dragPointer(page, hGateCenter, { x: placedGateCenter.x + 80, y: placedGateCenter.y + 40 }, 6, false)
  await page.waitForTimeout(50)
  const duringOtherDrag = await sampleCanvasPixels(page, canvas, signaturePoints)

  let totalDiff = 0
  for (const name of Object.keys(beforeDrag)) {
    const before = beforeDrag[name]
    const during = duringOtherDrag[name]
    totalDiff += Math.abs(before[0] - during[0]) + Math.abs(before[1] - during[1]) + Math.abs(before[2] - during[2])
  }

  expect(totalDiff).toBeLessThan(1100)

  await page.mouse.up()
})

test('CNOT with control on q1 yields bell state', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY1 })

  const expectedAfterH = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0, 0, 0, 0, 0]
  await waitForStateVectorApprox(page, expectedAfterH)

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY1 })

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX2, y: targetY0 })

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expectedBell)
})

test('Control does not affect gates in other columns', async ({ page }) => {
  await page.goto('/')

  await page.waitForFunction(
    () => window.__eguiReady === true || Boolean(window.__eguiError),
    null,
    { timeout: 20000 }
  )
  const eguiError = await page.evaluate(() => window.__eguiError || null)
  expect(eguiError).toBeNull()

  await waitForStateVectorReady(page)

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const SLOT_SPACING = 1.5 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const CIRCUIT_PADDING = 2 * REM
  const QUBIT_LABEL_WIDTH = 3 * 14
  const QUBIT_LABEL_GAP = 12
  const LINE_LEFT_OFFSET = CIRCUIT_PADDING + QUBIT_LABEL_WIDTH + QUBIT_LABEL_GAP
  const LINE_Y = 6.5 * REM
  const LINE_GAP = 1.5 * REM
  const PALETTE_COUNT = 15
  const paletteWidth = PALETTE_COUNT * PALETTE_SIZE + (PALETTE_COUNT - 1) * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const paletteCenterX = (index) =>
    startX + index * (PALETTE_SIZE + PALETTE_GAP) + PALETTE_SIZE / 2

  const sourceX = paletteCenterX(0)
  const controlX = paletteCenterX(1)
  const xGateX = paletteCenterX(2)
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetX2 = targetX + SLOT_SPACING
  const targetX3 = targetX2 + SLOT_SPACING
  const targetY0 = LINE_Y
  const targetY1 = LINE_Y + LINE_GAP

  await dragPointer(page, { x: sourceX, y: sourceY }, { x: targetX, y: targetY0 })

  await dragPointer(page, { x: controlX, y: sourceY }, { x: targetX2, y: targetY0 })

  await dragPointer(page, { x: xGateX, y: sourceY }, { x: targetX3, y: targetY1 })

  const expected = [0, 0, 1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0]
  await waitForStateVectorApprox(page, expected)
})

test('default chromium shows a visible WebGPU error instead of a blank page', async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_PATH || chromium.executablePath(),
    args: ['--disable-gpu', '--disable-software-rasterizer'],
  })

  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 800 } })
    await page.goto('http://127.0.0.1:4174/', { waitUntil: 'load' })
    await page.waitForFunction(
      () => window.__eguiReady === true || Boolean(window.__eguiError),
      null,
      { timeout: 20000 }
    )

    await expect.poll(async () => page.evaluate(() => window.__eguiError || null), {
      timeout: 20000,
    }).not.toBeNull()
    await expect(page.locator('[data-testid="webgpu-error"]')).toBeVisible()
    await expect(page.locator('[data-testid="webgpu-error"]')).toContainText('WebGPU')
  } finally {
    await browser.close()
  }
})
