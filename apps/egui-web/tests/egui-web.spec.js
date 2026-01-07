const { test, expect } = require('@playwright/test')

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

  const initialState = await page.evaluate(() =>
    window.__eguiReadStateVector ? window.__eguiReadStateVector() : []
  )
  expect(initialState).toEqual([1, 0, 0, 0])
  const stateCount = Math.max(1, initialState.length / 2)

  const initialScreenshot = await canvas.screenshot({ type: 'png', path: '/tmp/qni-egui-webgpu-initial.png' })
  const initialBase64 = initialScreenshot.toString('base64')
  const initialColor = await page.evaluate(
    async ({ base64, cssWidth, cssHeight, stateCount }) => {
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

      const rem = 32
      const size = 1.25 * rem
      const gap = 0.5 * rem
      const count = stateCount
      const bottomMargin = 2 * rem
      const totalWidth = count * size + (count - 1) * gap
      const baseX = (cssWidth - totalWidth) / 2
      const baseY = cssHeight - bottomMargin - size
      const centerX = baseX + size / 2
      const centerY = baseY + size / 2
      const scaleX = canvas.width / cssWidth
      const scaleY = canvas.height / cssHeight
      const px = Math.max(0, Math.min(canvas.width - 1, Math.round(centerX * scaleX)))
      const py = Math.max(0, Math.min(canvas.height - 1, Math.round(centerY * scaleY)))
      const idx = (py * canvas.width + px) * 4
      return { r: data[idx], g: data[idx + 1], b: data[idx + 2] }
    },
    { base64: initialBase64, cssWidth, cssHeight, stateCount }
  )
  expect(initialColor.r).toBeGreaterThan(30)
  expect(initialColor.g).toBeGreaterThan(120)
  expect(initialColor.b).toBeGreaterThan(160)

  const REM = 32
  const GATE_SIZE = 1 * REM
  const PALETTE_SIZE = GATE_SIZE
  const PALETTE_GAP = 0.5 * REM
  const PALETTE_ROW_Y = 2 * REM
  const LINE_LEFT_OFFSET = 2 * REM
  const LINE_Y = 6.5 * REM
  const paletteWidth = 9 * PALETTE_SIZE + 8 * PALETTE_GAP
  const startX = cssWidth / 2 - paletteWidth / 2
  const sourceX = startX + PALETTE_SIZE / 2
  const sourceY = PALETTE_ROW_Y + PALETTE_SIZE / 2
  const targetX = LINE_LEFT_OFFSET + GATE_SIZE
  const targetY = LINE_Y

  await page.mouse.move(sourceX + offsetX, sourceY + offsetY)
  await page.mouse.down()
  await page.mouse.move(targetX + offsetX, targetY + offsetY, { steps: 6 })
  await page.mouse.up()

  const expected = [1 / Math.sqrt(2), 0, 1 / Math.sqrt(2), 0]
  await page.waitForFunction(
    (expectedState) => {
      const actual = window.__eguiReadStateVector ? window.__eguiReadStateVector() : []
      if (actual.length !== expectedState.length) {
        return false
      }
      return expectedState.every((value, index) => Math.abs(actual[index] - value) < 1e-3)
    },
    expected,
    { timeout: 5000 }
  )

  const screenshot = await canvas.screenshot({ type: 'png', path: '/tmp/qni-egui-webgpu-after.png' })
  const base64 = screenshot.toString('base64')

  const stats = await page.evaluate(async ({ base64 }) => {
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
