import type { Gate } from '../domain/gate'

export type Color = [number, number, number, number]

export type TextLayout = {
  text: string
  x: number
  y: number
  color: Color
  glyphCount?: number
}

export type ShapeInstance = {
  kind: 0 | 1 | 2
  thickness: number
  p0x: number
  p0y: number
  p1x: number
  p1y: number
  color: Color
}

export type PlacedGate = {
  id: number
  x: number
  y: number
  wire: number
  label: Gate
  dragging: boolean
  hovered: boolean
}
