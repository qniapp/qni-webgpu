import { expect, test, type Page } from '@playwright/test'
import { waitForStartupReady, sampleCanvasPixels } from '../features/support/egui-helpers'

export const TUTORIAL_URL = 'https://qniapp.github.io/qni-tutorial-webgpu/'
export type Rect = { left: number; right: number; top: number; bottom: number }
const hook = (name: string) => `__qni${name}GeometryJson`
export const rectangle = async (page: Page, name: string): Promise<Rect | null> =>
  page.evaluate(key => {
    const raw = (window as any)[key]
    return typeof raw === 'string' && raw ? JSON.parse(raw) : null
  }, hook(name))
export const waitForRectangle = async (page: Page, name: string): Promise<Rect> => {
  const value = await page.waitForFunction(key => {
    const raw = (window as any)[key]
    return typeof raw === 'string' && raw.length > 0 ? JSON.parse(raw) : null
  }, hook(name))
  return value.jsonValue()
}
const overflow = async (page: Page): Promise<{ stage: string; overflow: string[] }> =>
  page.evaluate(() => {
    const raw = (window as any).__qniToolbarOverflowJson
    return typeof raw === 'string' ? JSON.parse(raw) : { stage: 'missing', overflow: [] }
  })

export const settleToolbar = async (page: Page) => {
  // Layout hooks are published by egui, not by the browser resize event.
  await page.waitForTimeout(120)
  await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
}
export const clickRectangle = async (page: Page, rect: Rect | null) => {
  if (!rect) throw new Error('Missing toolbar rectangle')
  await page.locator('#egui-canvas').click({ position: { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 } })
  await settleToolbar(page)
}

export const revealTutorial = async (page: Page): Promise<Rect> => {
  let rect = await rectangle(page, 'ToolbarTutorial')
  if (!rect) {
    await clickRectangle(page, await rectangle(page, 'ToolbarMore'))
    rect = await waitForRectangle(page, 'ToolbarMoreTutorial')
  }
  if (!rect) throw new Error('Tutorial is neither on the toolbar nor in More')
  return rect
}

const inViewport = (rect: Rect, width: number, height: number) =>
  rect.left >= 0 && rect.top >= 0 && rect.right <= width + 0.5 && rect.bottom <= height + 0.5 && rect.right > rect.left && rect.bottom > rect.top

const WIDTHS = [320, 360, 390, 480, 600, 768, 900, 1024, 1280, 1440]
const circuitUrl = (gpu = false, title = 'Circuit 1') => (gpu ? '/?mode=gpu' : '/') + '#' + encodeURIComponent(JSON.stringify({ title, cols: [['H']] }))
const storage = async (page: Page): Promise<any> => page.evaluate(() => {
  const raw = localStorage.getItem('qni.circuit_library.v2')
  if (!raw) throw new Error('Circuit library v2 was not persisted')
  return JSON.parse(raw)
})
const activeEntry = async (page: Page) => {
  const library = await storage(page)
  return library.entries.find((entry: any) => entry.id === library.active_id)
}
const columns = (page: Page) => {
  const hash = new URL(page.url()).hash.slice(1)
  return hash ? JSON.parse(decodeURIComponent(hash)).cols : []
}
const clickTool = async (page: Page, name: string) => clickRectangle(page, await waitForRectangle(page, `Toolbar${name}`))
const mode = (page: Page) => new URL(page.url()).searchParams.get('mode')
const chooseMode = async (page: Page, next: 'local' | 'gpu', close = true) => {
  const name = next === 'gpu' ? 'ToolbarExecGpu' : 'ToolbarExecLocal'
  if (!await rectangle(page, name)) await clickTool(page, 'More')
  await clickRectangle(page, await waitForRectangle(page, name))
  if (close) {
    await page.keyboard.press('Escape')
    await settleToolbar(page)
  }
}
const prepare = async (page: Page, width: number, resized: boolean, options: { height?: number; gpu?: boolean; title?: string } = {}) => {
  const height = options.height ?? 800
  await page.setViewportSize({ width: resized ? 1440 : width, height })
  await page.goto(circuitUrl(options.gpu, options.title))
  await waitForStartupReady(page)
  if (options.title && options.title !== 'Circuit 1') {
    // A circuit title does not rename its library entry (the picker uses the latter).
    await page.evaluate(name => {
      const library = JSON.parse(localStorage.getItem('qni.circuit_library.v2')!)
      library.entries.find((entry: any) => entry.id === library.active_id).name = name
      localStorage.setItem('qni.circuit_library.v2', JSON.stringify(library))
      history.replaceState(null, '', location.pathname + location.search)
    }, options.title)
    await page.reload()
    await waitForStartupReady(page)
  }
  if (resized) await page.setViewportSize({ width, height })
  await settleToolbar(page)
}
const statuses = [
  { status: 'running' },
  { status: 'completed', durationMs: 1400 },
  { status: 'failed', failure: 'offline', url: 'http://127.0.0.1:4184/run' },
]
const setStatus = async (page: Page, status: object, width: number, resized: boolean) => {
  if (resized) {
    await page.setViewportSize({ width: 1440, height: 800 })
    await settleToolbar(page)
  }
  await page.evaluate(value => (window as any).__setExternalGpuStatus(JSON.stringify(value)), status)
  await settleToolbar(page)
  if (resized) {
    await page.setViewportSize({ width, height: 800 })
    await settleToolbar(page)
  }
}
const tutorialPopup = async (page: Page, keyboard?: string) => {
  await page.context().route('https://qniapp.github.io/**', route => route.fulfill({ contentType: 'text/html', body: '<title>Tutorial</title>' }))
  const rect = keyboard ? null : await revealTutorial(page)
  const popupPromise = page.waitForEvent('popup')
  if (keyboard) await page.keyboard.press(keyboard)
  else await clickRectangle(page, rect)
  const popup = await popupPromise
  await popup.waitForLoadState('domcontentloaded')
  return popup
}

// Geometry readers return measurements only. Each test checks one invariant.
const toolbarGeometry = async (page: Page, gpu: boolean, pages: boolean) => {
  const names = ['Picker', 'Undo', 'Redo', 'Clear', 'Duplicate', 'Lock', ...(gpu ? ['Run'] : [])]
  const required = await Promise.all(names.map(name => rectangle(page, `Toolbar${name}`)))
  const conditional = await Promise.all(['GpuStatus', 'Tutorial', 'More', ...(!pages ? ['ExecLocal', 'ExecGpu'] : [])].map(name => rectangle(page, `Toolbar${name}`)))
  const visible = [...required, ...conditional.filter((rect): rect is Rect => rect !== null)]
  const lock = required[5]
  const compact = Boolean(lock && lock.right - lock.left < 28)
  const [status, tutorial, more, local, gpuToggle] = conditional
  return {
    visible,
    gap: Math.min(...[tutorial?.left, more?.left, local ? local.left - 2 : undefined].filter((x): x is number => x !== undefined))
      - Math.max(...required.map(rect => rect?.right ?? 0), status?.right ?? 0),
    minimumGap: compact ? 8 : 16,
    anchor: more?.right ?? (gpuToggle ? gpuToggle.right + 2 : tutorial?.right),
    padding: compact ? 8 : 12,
  }
}
export function registerToolbarReachability({ pages }: { pages: boolean }) {
  for (const width of WIDTHS) for (const resized of [false, true]) {
    test.describe(`toolbar ${pages ? 'Pages' : 'ABCI'} ${width}px ${resized ? 'from 1440' : 'fresh'}`, () => {
      test.beforeEach(async ({ page }) => prepare(page, width, resized))
      test('Local controls are inside the viewport', async ({ page }) => {
        const geometry = await toolbarGeometry(page, false, pages)
        expect(geometry.visible.every(rect => rect !== null && inViewport(rect, width, 800))).toBe(true)
      })
      test('Local left and right groups retain GAP_MIN', async ({ page }) => {
        const geometry = await toolbarGeometry(page, false, pages)
        expect(geometry.gap).toBeGreaterThanOrEqual(geometry.minimumGap - 0.5)
      })
      test('right group is anchored to the current viewport', async ({ page }) => {
        const geometry = await toolbarGeometry(page, false, pages)
        expect(geometry.anchor).toBeCloseTo(width - geometry.padding, 0)
      })
      test('picker click opens the circuit list', async ({ page }) => {
        await clickTool(page, 'Picker')
        expect(await page.evaluate(() => Boolean((window as any).__qniCircuitPickerDropdownGeometryJson))).toBe(true)
      })
      test('Clear removes gates', async ({ page }) => {
        await clickTool(page, 'Clear')
        expect(columns(page)).toEqual([])
      })
      test('Undo restores cleared gates', async ({ page }) => {
        await clickTool(page, 'Clear')
        await clickTool(page, 'Undo')
        expect(columns(page)).toEqual([['H']])
      })
      test('Redo reapplies Clear', async ({ page }) => {
        await clickTool(page, 'Clear')
        await clickTool(page, 'Undo')
        await clickTool(page, 'Redo')
        expect(columns(page)).toEqual([])
      })
      test('Duplicate inserts another circuit', async ({ page }) => {
        const before = await storage(page)
        await clickTool(page, 'Duplicate')
        expect((await storage(page)).entries.length).toBe(before.entries.length + 1)
      })


      test('Lock persists locked state', async ({ page }) => {
        await clickTool(page, 'Lock')
        expect((await activeEntry(page)).origin.locked).toBe(true)
      })
      test('Unlock persists unlocked state', async ({ page }) => {
        await clickTool(page, 'Lock')
        await clickTool(page, 'Lock')
        expect((await activeEntry(page)).origin.locked).toBe(false)
      })
      test('Tutorial click opens the exact popup URL', async ({ page }) => {
        const popup = await tutorialPopup(page)
        await expect(popup).toHaveURL(TUTORIAL_URL)
      })
      if (pages) return
      test('GPU selector writes mode URL', async ({ page }) => {
        await chooseMode(page, 'gpu')
        expect(mode(page)).toBe('gpu')
      })
      test('Local selector removes mode URL', async ({ page }) => {
        await chooseMode(page, 'gpu')
        await chooseMode(page, 'local')
        expect(mode(page)).toBeNull()
      })

      for (const invariant of ['viewport', 'gap'] as const) {
        test(`GPU idle ${invariant}`, async ({ page }) => {
          await chooseMode(page, 'gpu')
          if (resized) {
            await page.setViewportSize({ width: 1440, height: 800 })
            await settleToolbar(page)
            await page.setViewportSize({ width, height: 800 })
            await settleToolbar(page)
          }
          const geometry = await toolbarGeometry(page, true, false)
          const holds = invariant === 'viewport'
            ? geometry.visible.every(rect => rect !== null && inViewport(rect, width, 800))
            : geometry.gap >= geometry.minimumGap - 0.5
          expect(holds).toBe(true)
        })
      }
      test('Run invokes the backend stub once', async ({ page }) => {
        await chooseMode(page, 'gpu')
        await page.evaluate(() => {
          (window as any).__toolbarRunCalls = 0
          ;(window as any).__qniRunQiskitBackend = async () => {
            (window as any).__toolbarRunCalls++
            return { status: 'completed', runner: 'test', qubits: 1, shots: 1024, histogram: { '0': 1024 }, truncated: false }
          }
        })
        await clickTool(page, 'Run')
        expect(await page.evaluate(() => (window as any).__toolbarRunCalls)).toBe(1)
      })
      for (const status of statuses) {
        for (const invariant of ['viewport', 'gap'] as const) {
          test(`GPU ${status.status} ${invariant}`, async ({ page }) => {
            await chooseMode(page, 'gpu')
            await setStatus(page, status, width, resized)
            const geometry = await toolbarGeometry(page, true, false)
            const holds = invariant === 'viewport'
              ? geometry.visible.every(rect => rect !== null && inViewport(rect, width, 800))
              : geometry.gap >= geometry.minimumGap - 0.5
            expect(holds).toBe(true)
          })
        }
        test(`GPU ${status.status} indicator is hoverable`, async ({ page }) => {
          await chooseMode(page, 'gpu')
          await setStatus(page, status, width, resized)
          const indicator = await waitForRectangle(page, 'ToolbarGpuStatus')
          await page.locator('#egui-canvas').hover({ position: { x: (indicator.left + indicator.right) / 2, y: (indicator.top + indicator.bottom) / 2 } })
          await settleToolbar(page)
          expect((await rectangle(page, 'ToolbarGpuStatus') as any)?.hovered).toBe(true)
        })
      }
      test('Local clears the Run rectangle', async ({ page }) => {
        await chooseMode(page, 'gpu')
        await chooseMode(page, 'local')
        expect(await rectangle(page, 'ToolbarRun')).toBeNull()
      })
      test('Local clears the status rectangle', async ({ page }) => {
        await chooseMode(page, 'gpu')
        await setStatus(page, statuses[1], width, resized)
        await chooseMode(page, 'local')
        expect(await rectangle(page, 'ToolbarGpuStatus')).toBeNull()
      })
    })
  }
  registerToolbarEdgeCases(pages)
}

const LONG_NAME = 'Long circuit name '.repeat(12)
const menuOpen = async (page: Page) => (await rectangle(page, 'ToolbarMoreTutorial')) !== null
const openByKey = async (page: Page, key = 'Enter') => {
  await page.keyboard.press('Tab')
  await page.keyboard.press(key)
  await settleToolbar(page)
}
const positions = async (page: Page) => {
  const result: Record<string, Rect | null> = {}
  for (const name of ['Lock', 'Run', 'GpuStatus', 'More', 'Tutorial', 'MoreTutorial', 'ExecLocal', 'ExecGpu']) {
    const rect = await rectangle(page, `Toolbar${name}`)
    result[name] = rect ? { left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom } : null
  }
  return result
}

function registerToolbarEdgeCases(pages: boolean) {
  test('More popup paints above the Local state-panel header', async ({ page }) => {
    await prepare(page, pages ? 390 : 320, false, { height: pages ? 300 : 800, title: LONG_NAME })
    await clickTool(page, 'More')
    const menu = await waitForRectangle(page, 'ToolbarMoreMenu')
    const pixels = await sampleCanvasPixels(page, page.locator('#egui-canvas'), [{ name: 'menu', x: menu.left + 12, y: menu.top + 10 }])
    const distance = Math.hypot(...pixels.menu.slice(0, 3).map((channel, i) => channel - [255, 252, 240][i]))
    expect(distance).toBeLessThan(8)
  })

  if (pages) {
    test('standard-density Tab then Enter opens More', async ({ page }) => {
      await prepare(page, 500, false, { height: 320, title: LONG_NAME })
      const more = await waitForRectangle(page, 'ToolbarMore')
      if (more.right - more.left < 31) throw new Error('Fixture requires standard-density More')
      await openByKey(page)
      expect(await menuOpen(page)).toBe(true)
    })
  } else {
    test('standard-density overflow ArrowRight selects GPU', async ({ page }) => {
      await prepare(page, 500, false, { height: 300, title: LONG_NAME })
      const more = await waitForRectangle(page, 'ToolbarMore')
      if (more.right - more.left < 31) throw new Error('Fixture requires standard-density More')
      if (!(await overflow(page)).overflow.includes('ExecMode')) throw new Error('Fixture requires the toggle in More')
      await openByKey(page)
      await page.keyboard.press('ArrowRight')
      await settleToolbar(page)
      expect(mode(page)).toBe('gpu')
    })
  }

  test('Duplicate activates the new circuit', async ({ page }) => {
    await prepare(page, 390, false)
    const before = await storage(page)
    await clickTool(page, 'Duplicate')
    expect((await storage(page)).active_id).not.toBe(before.active_id)
  })
  test('Duplicate preserves gate content', async ({ page }) => {
    await prepare(page, 390, false)
    await clickTool(page, 'Duplicate')
    expect(JSON.parse((await activeEntry(page)).circuit_json).cols).toEqual([['H']])
  })
  if (!pages) test('mode switching preserves circuit hash', async ({ page }) => {
    await prepare(page, 320, false)
    const hash = new URL(page.url()).hash
    await chooseMode(page, 'gpu')
    await chooseMode(page, 'local')
    expect(new URL(page.url()).hash).toBe(hash)
  })

  test.describe('overflow interactions', () => {
    test.beforeEach(async ({ page }) => prepare(page, 320, false, { height: pages ? 240 : 800, title: LONG_NAME }))
    for (const key of ['Enter', 'Space', 'ArrowDown']) {
      test(`Tab then ${key} opens More`, async ({ page }) => {
        await openByKey(page, key)
        expect(await menuOpen(page)).toBe(true)
      })
    }
    test('Escape closes More', async ({ page }) => {
      await openByKey(page)
      await page.keyboard.press('Escape')
      await settleToolbar(page)
      expect(await menuOpen(page)).toBe(false)
    })
    test('Escape restores focus to More for keyboard reopening', async ({ page }) => {
      await openByKey(page)
      await page.keyboard.press('Escape')
      await settleToolbar(page)
      await page.keyboard.press('Enter')
      await settleToolbar(page)
      expect(await menuOpen(page)).toBe(true)
    })
    test('ArrowUp wraps from the first row to Tutorial', async ({ page }) => {
      await openByKey(page)
      await page.keyboard.press('ArrowUp')
      const popup = await tutorialPopup(page, 'Space')
      await expect(popup).toHaveURL(TUTORIAL_URL)
    })
    test('ArrowDown navigates to Tutorial', async ({ page }) => {
      await openByKey(page)
      await page.keyboard.press('ArrowDown')
      const popup = await tutorialPopup(page, 'Enter')
      await expect(popup).toHaveURL(TUTORIAL_URL)
    })
    test('ArrowDown wraps after the final row', async ({ page }) => {
      await openByKey(page)
      await page.keyboard.press('ArrowDown')
      await settleToolbar(page)
      if (!pages) {
        await page.keyboard.press('ArrowDown')
        await settleToolbar(page)
      }
      let selected: string | null
      if (pages) {
        const popup = await tutorialPopup(page, 'Enter')
        selected = popup.url()
      } else {
        await page.keyboard.press('ArrowRight')
        await settleToolbar(page)
        selected = mode(page)
      }
      expect(selected).toBe(pages ? TUTORIAL_URL : 'gpu')
    })
    test('Tutorial activation closes More', async ({ page }) => {
      const popup = await tutorialPopup(page)
      await popup.close()
      await page.bringToFront()
      await settleToolbar(page)
      expect(await menuOpen(page)).toBe(false)
    })
    test('keyboard Tutorial activation closes More', async ({ page }) => {
      await openByKey(page)
      await page.keyboard.press('ArrowUp')
      const popup = await tutorialPopup(page, 'Space')
      await popup.close()
      await page.bringToFront()
      await settleToolbar(page)
      expect(await menuOpen(page)).toBe(false)
    })
    test('opening picker closes More', async ({ page }) => {
      await clickTool(page, 'More')
      await clickTool(page, 'Picker')
      expect(await menuOpen(page)).toBe(false)
    })
    test('picker opens while closing More', async ({ page }) => {
      await clickTool(page, 'More')
      await clickTool(page, 'Picker')
      expect(await page.evaluate(() => Boolean((window as any).__qniCircuitPickerDropdownGeometryJson))).toBe(true)
    })
    test('opening More clears picker dropdown geometry', async ({ page }) => {
      await clickTool(page, 'Picker')
      await clickTool(page, 'More')
      expect(await page.evaluate(() => Boolean((window as any).__qniCircuitPickerDropdownGeometryJson))).toBe(false)
    })
    test('More opens while closing picker', async ({ page }) => {
      await clickTool(page, 'Picker')
      await clickTool(page, 'More')
      expect(await menuOpen(page)).toBe(true)
    })
    test('outside click closes More', async ({ page }) => {
      await clickTool(page, 'More')
      await page.locator('#egui-canvas').click({ position: { x: 20, y: pages ? 220 : 500 } })
      await settleToolbar(page)
      expect(await menuOpen(page)).toBe(false)
    })
    test('growth clears the More button rectangle', async ({ page }) => {
      await clickTool(page, 'More')
      await page.setViewportSize({ width: 1440, height: 800 })
      await settleToolbar(page)
      expect(await rectangle(page, 'ToolbarMore')).toBeNull()
    })
    test('growth closes the now-empty overflow', async ({ page }) => {
      await clickTool(page, 'More')
      await page.setViewportSize({ width: 1440, height: 800 })
      await settleToolbar(page)
      expect(await rectangle(page, 'ToolbarMoreMenu')).toBeNull()
    })
    test('growth leaves no displaced items', async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 800 })
      await settleToolbar(page)
      expect((await overflow(page)).overflow).toEqual([])
    })
    test('grow then re-shrink restores fresh geometry', async ({ page }) => {
      const fresh = await positions(page)
      await clickTool(page, 'More')
      await page.setViewportSize({ width: 1440, height: 800 })
      await settleToolbar(page)
      await page.setViewportSize({ width: 320, height: pages ? 240 : 800 })
      await settleToolbar(page)
      expect(await positions(page)).toEqual(fresh)
    })
    if (!pages) {
      test('overflow ArrowRight selects GPU', async ({ page }) => {
        await openByKey(page)
        await page.keyboard.press('ArrowRight')
        await settleToolbar(page)
        expect(mode(page)).toBe('gpu')
      })
      test('overflow ArrowLeft selects Local', async ({ page }) => {
        await openByKey(page)
        await page.keyboard.press('ArrowRight')
        await settleToolbar(page)
        await page.keyboard.press('ArrowLeft')
        await settleToolbar(page)
        expect(mode(page)).toBeNull()
      })
      test('changing overflow mode keeps More open', async ({ page }) => {
        await chooseMode(page, 'gpu', false)
        expect(await menuOpen(page)).toBe(true)
      })
      test('Escape clears hidden overflow toggle rectangles', async ({ page }) => {
        await clickTool(page, 'More')
        await page.keyboard.press('Escape')
        await settleToolbar(page)
        const rows = await Promise.all(['ToolbarExecLocal', 'ToolbarExecGpu'].map(name => rectangle(page, name)))
        expect(rows.every(rect => rect === null)).toBe(true)
      })
    }
  })

  test.describe('long-name short landscape fallback', () => {
    test.beforeEach(async ({ page }) => {
      await prepare(page, 320, false, { height: 240, gpu: !pages, title: LONG_NAME })
      if (!pages) {
        await page.evaluate(value => (window as any).__setExternalGpuStatus(JSON.stringify(value)), statuses[2])
        await settleToolbar(page)
      }
    })
    test('selects S4', async ({ page }) => {
      expect((await overflow(page)).stage).toBe('S4')
    })
    test('uses compact editing buttons', async ({ page }) => {
      const lock = await waitForRectangle(page, 'ToolbarLock')
      expect(lock.right - lock.left).toBe(24)
    })
    test('controls stay inside the viewport', async ({ page }) => {
      const geometry = await toolbarGeometry(page, !pages, pages)
      expect(geometry.visible.every(rect => rect !== null && inViewport(rect, 320, 240))).toBe(true)
    })
    test('left and right groups retain GAP_MIN', async ({ page }) => {
      const geometry = await toolbarGeometry(page, !pages, pages)
      expect(geometry.gap).toBeGreaterThanOrEqual(geometry.minimumGap - 0.5)
    })
    test('overflow popup fits in the viewport', async ({ page }) => {
      await revealTutorial(page)
      const menu = await waitForRectangle(page, 'ToolbarMoreMenu')
      expect(inViewport(menu, 320, 240)).toBe(true)
    })
  })
  if (!pages) for (const width of [768, 900]) {
    test(`failed GPU ${width}px overflow Tutorial opens the exact popup URL`, async ({ page }) => {
      await prepare(page, width, false, { gpu: true })
      await setStatus(page, statuses[2], width, false)
      const popup = await tutorialPopup(page)
      await expect(popup).toHaveURL(TUTORIAL_URL)
    })
  }
  if (pages) {
    test('Pages 480x320 long-name S1 Tutorial opens the exact popup URL', async ({ page }) => {
      await prepare(page, 480, false, { height: 320, title: LONG_NAME })
      const popup = await tutorialPopup(page)
      await expect(popup).toHaveURL(TUTORIAL_URL)
    })
    test('Pages S0 leaves Tab to native focus navigation', async ({ page }) => {
      await prepare(page, 1440, false)
      await page.evaluate(() => {
        for (const id of ['before', 'after']) {
          const button = document.createElement('button')
          button.id = id
          button.textContent = id
          document.body.append(button)
        }
        document.getElementById('before')!.focus()
      })
      await page.keyboard.press('Tab')
      expect(await page.evaluate(() => document.activeElement?.id)).toBe('after')
    })
  }
}
