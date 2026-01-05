import { CANVAS_WIDTH, GATE_SIZE, LINE_Y, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE, SLOT_COUNT, SLOT_LEFT, SLOT_SPACING, SLOT_RIGHT, SNAP_DISTANCE } from './constants'
import type { PlacedGate } from './types'

type InputOptions = {
  canvas: HTMLCanvasElement
  placedGates: PlacedGate[]
  onUpdate: () => void
  onGateDropped: (gate: PlacedGate) => void
}

export function setupInput({ canvas, placedGates, onUpdate, onGateDropped }: InputOptions) {
  let nextGateId = 1
  let activeGateId: number | null = null
  let dragOffsetX = 0
  let dragOffsetY = 0

  const paletteWidth = PALETTE_GATES.length * PALETTE_SIZE + (PALETTE_GATES.length - 1) * PALETTE_GAP
  const paletteStartX = (CANVAS_WIDTH - paletteWidth) / 2
  const slotCenters = Array.from({ length: SLOT_COUNT }, (_, index) => SLOT_LEFT + SLOT_SPACING * index)

  const nearestSlotCenter = (x: number) => {
    let nearest = slotCenters[0] ?? x
    let nearestDistance = Math.abs(x - nearest)
    for (const slot of slotCenters) {
      const distance = Math.abs(x - slot)
      if (distance < nearestDistance) {
        nearest = slot
        nearestDistance = distance
      }
    }
    return nearest
  }

  const getPointerPosition = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect()
    const scaleX = canvas.width / rect.width
    const scaleY = canvas.height / rect.height
    return {
      x: (event.clientX - rect.left) * scaleX,
      y: (event.clientY - rect.top) * scaleY,
    }
  }

  canvas.addEventListener('pointerdown', (event) => {
    const { x, y } = getPointerPosition(event)
    const hitGate = [...placedGates].reverse().find((gate) => x >= gate.x && x <= gate.x + GATE_SIZE && y >= gate.y && y <= gate.y + GATE_SIZE)
    if (hitGate) {
      activeGateId = hitGate.id
      hitGate.dragging = true
      dragOffsetX = x - hitGate.x
      dragOffsetY = y - hitGate.y
      onUpdate()
      canvas.setPointerCapture(event.pointerId)
      return
    }

    if (y >= PALETTE_ROW_Y && y <= PALETTE_ROW_Y + PALETTE_SIZE && x >= paletteStartX && x <= paletteStartX + paletteWidth) {
      const localX = x - paletteStartX
      const index = Math.floor(localX / (PALETTE_SIZE + PALETTE_GAP))
      const inBox = localX - index * (PALETTE_SIZE + PALETTE_GAP) <= PALETTE_SIZE
      if (index >= 0 && index < PALETTE_GATES.length && inBox) {
        const newGate: PlacedGate = {
          id: nextGateId++,
          x: x - GATE_SIZE / 2,
          y: y - GATE_SIZE / 2,
          label: PALETTE_GATES[index],
          dragging: true,
        }
        placedGates.push(newGate)
        activeGateId = newGate.id
        dragOffsetX = GATE_SIZE / 2
        dragOffsetY = GATE_SIZE / 2
        onUpdate()
        canvas.setPointerCapture(event.pointerId)
      }
    }
  })

  canvas.addEventListener('pointermove', (event) => {
    if (activeGateId === null) {
      return
    }
    const gate = placedGates.find((item) => item.id === activeGateId)
    if (!gate) {
      return
    }
    const { x, y } = getPointerPosition(event)
    gate.x = x - dragOffsetX
    const nextY = y - dragOffsetY
    const snapY = LINE_Y - GATE_SIZE / 2
    const centerY = nextY + GATE_SIZE / 2
    const shouldSnapY = Math.abs(centerY - LINE_Y) <= SNAP_DISTANCE
    if (shouldSnapY) {
      gate.y = snapY
      const centerX = x - dragOffsetX + GATE_SIZE / 2
      const snappedCenterX = nearestSlotCenter(centerX)
      gate.x = snappedCenterX - GATE_SIZE / 2
    } else {
      gate.y = nextY
    }
    onUpdate()
  })

  const handlePointerEnd = (event: PointerEvent) => {
    if (activeGateId === null) {
      return
    }
    canvas.releasePointerCapture(event.pointerId)
    const gateIndex = placedGates.findIndex((item) => item.id === activeGateId)
    const gate = gateIndex >= 0 ? placedGates[gateIndex] : null
    if (!gate) {
      activeGateId = null
      return
    }
    gate.dragging = false
    const centerX = gate.x + GATE_SIZE / 2
    const centerY = gate.y + GATE_SIZE / 2
    const onCircuit =
      centerX >= SLOT_LEFT &&
      centerX <= SLOT_RIGHT &&
      Math.abs(centerY - LINE_Y) <= SNAP_DISTANCE
    if (!onCircuit) {
      placedGates.splice(gateIndex, 1)
    } else {
      const snappedCenterX = nearestSlotCenter(centerX)
      gate.x = snappedCenterX - GATE_SIZE / 2
      gate.y = LINE_Y - GATE_SIZE / 2
      onGateDropped(gate)
    }
    activeGateId = null
    onUpdate()
  }

  canvas.addEventListener('pointerup', handlePointerEnd)
  canvas.addEventListener('pointercancel', handlePointerEnd)
}
