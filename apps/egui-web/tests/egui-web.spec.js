const { test, expect } = require('@playwright/test')

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
  const offsetX = box?.x ?? 0
  const offsetY = box?.y ?? 0
  const cssWidth = box?.width ?? (viewport?.width ?? 1000)
  const cssHeight = box?.height ?? (viewport?.height ?? 800)

  await page.waitForFunction(
    async () => {
      if (!window.__eguiReadStateVector) {
        return false
      }
      const state = await window.__eguiReadStateVector()
      return state.length > 0
    },
    null,
    { timeout: 20000 }
  )
  const initialState = await page.evaluate(async () =>
    window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
  )
  expect(initialState).toEqual([1, 0, 0, 0])
  const stateCount = Math.max(1, initialState.length / 2)

  await page.waitForTimeout(500)
  const initialScreenshot = await canvas.screenshot({ type: 'png', path: '/tmp/qni-egui-webgpu-initial.png' })
  const initialBase64 = initialScreenshot.toString('base64')
  const initialColor = await evaluateWithRetry(
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
        return { r: 0, g: 0, b: 0 }
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
    { base64: initialBase64 }
  )
  expect(initialColor.hits).toBeGreaterThan(100)

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

  await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX + offsetX, targetY + offsetY, { steps: 6 })
  await page.mouse.up()

  const expected = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expected,
    { timeout: 5000 }
  )

  await page.mouse.move(controlX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX2 + offsetX, targetY + offsetY, { steps: 6 })
  await page.mouse.up()

  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expected,
    { timeout: 5000 }
  )

  await page.mouse.move(xGateX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX2 + offsetX, targetY2 + offsetY, { steps: 6 })
  await page.mouse.up()

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 1 / Math.sqrt(2)]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expectedBell,
    { timeout: 5000 }
  )

  const screenshot = await canvas.screenshot({ type: 'png', path: '/tmp/qni-egui-webgpu-after.png' })
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

  await page.waitForFunction(
    async () => {
      if (!window.__eguiReadStateVector) {
        return false
      }
      const state = await window.__eguiReadStateVector()
      return state.length > 0
    },
    null,
    { timeout: 20000 }
  )

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()

  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const offsetX = box?.x ?? 0
  const offsetY = box?.y ?? 0
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

  await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX + offsetX, targetY0 + offsetY, { steps: 6 })
  await page.mouse.up()

  const expectedAfterQ0 = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expectedAfterQ0,
    { timeout: 5000 }
  )

  await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX + offsetX, targetY1 + offsetY, { steps: 6 })
  await page.mouse.up()

  const expected = [0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expected,
    { timeout: 5000 }
  )
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

  await page.waitForFunction(
    async () => {
      if (!window.__eguiReadStateVector) {
        return false
      }
      const state = await window.__eguiReadStateVector()
      return state.length > 0
    },
    null,
    { timeout: 20000 }
  )

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const offsetX = box?.x ?? 0
  const offsetY = box?.y ?? 0
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

  await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX + offsetX, targetY1 + offsetY, { steps: 6 })
  await page.mouse.up()

  const expectedAfterH = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0, 0, 0, 0, 0]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expectedAfterH,
    { timeout: 5000 }
  )

  await page.mouse.move(controlX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX2 + offsetX, targetY1 + offsetY, { steps: 6 })
  await page.mouse.up()

  await page.mouse.move(xGateX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX2 + offsetX, targetY0 + offsetY, { steps: 6 })
  await page.mouse.up()

  const expectedBell = [1 / Math.sqrt(2), 0, 0, 0, 0, 0, 1 / Math.sqrt(2), 0]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expectedBell,
    { timeout: 5000 }
  )
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

  await page.waitForFunction(
    async () => {
      if (!window.__eguiReadStateVector) {
        return false
      }
      const state = await window.__eguiReadStateVector()
      return state.length > 0
    },
    null,
    { timeout: 20000 }
  )

  const canvas = page.locator('#egui-canvas')
  await expect(canvas).toBeVisible()
  const viewport = page.viewportSize()
  const box = await canvas.boundingBox()
  expect(box).not.toBeNull()
  const offsetX = box?.x ?? 0
  const offsetY = box?.y ?? 0
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

  await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX + offsetX, targetY0 + offsetY, { steps: 6 })
  await page.mouse.up()

  await page.mouse.move(controlX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX2 + offsetX, targetY0 + offsetY, { steps: 6 })
  await page.mouse.up()

  await page.mouse.move(xGateX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX3 + offsetX, targetY1 + offsetY, { steps: 6 })
  await page.mouse.up()

  const expected = [0, 0, 1 / Math.sqrt(2), 0, 0, 0, 1 / Math.sqrt(2), 0]
  await page.waitForFunction(
    async (expectedState) => {
      const actual = window.__eguiReadStateVector ? await window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expected,
    { timeout: 5000 }
  )
})
