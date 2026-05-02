import type { Locator, Page } from 'playwright'

export type CanvasPixel = number[]

export type Point = {
  x: number
  y: number
}

export type PixelSamplePoint = Point & {
  name: string
}

export type DragPreviewProbe = {
  source: Point
  handleCenter: Point
  dragFillPoint: PixelSamplePoint
  sourceFillPoint: PixelSamplePoint
}

export type DragPreviewZOrderSamples = {
  before: CanvasPixel
  during: CanvasPixel
  source: CanvasPixel
}

export type EguiWorld = {
  page: Page | null
  baseUrl: string
  dragPreviewZOrder?: DragPreviewZOrderSamples
}

export type BrowserSupport = {
  STANDARD_BROWSER_MODE: string
  PLAIN_BROWSER_MODE: string
  openPageForMode: (world: EguiWorld, mode: string) => Promise<Page>
}

export type EguiHelpers = {
  openEguiApp: (page: Page, baseUrl: string) => Promise<void>
  waitForAppReady: (page: Page) => Promise<void>
  waitForStartupReady: (page: Page, options: { waitForStateVector: boolean }) => Promise<void>
  readEguiError: (page: Page) => Promise<string | null>
  readStateVector: (page: Page) => Promise<unknown>
  waitForCanvasContent: (page: Page, canvas: Locator) => Promise<void>
  waitForStateVectorReady: (page: Page) => Promise<void>
  dragPointer: (
    page: Page,
    from: Point,
    to: Point,
    steps?: number,
    release?: boolean
  ) => Promise<void>
  getDragPreviewAboveStatePanelProbe: (cssWidth: number, cssHeight: number) => DragPreviewProbe
  sampleCanvasPixels: (
    page: Page,
    locator: Locator,
    samples: PixelSamplePoint[]
  ) => Promise<Record<string, CanvasPixel>>
}

export type AssertionsSupport = {
  assertDragPreviewAboveOverlay: (samples: DragPreviewZOrderSamples) => void
}

export type WindowWithEguiError = Window & {
  __eguiError?: unknown
}
