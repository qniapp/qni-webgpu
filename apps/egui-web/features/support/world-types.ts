import type { Page } from 'playwright'

export type CanvasPixel = number[]

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
