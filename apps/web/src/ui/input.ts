import { CANVAS_WIDTH, GATE_SIZE, LINE_Y_VALUES, PALETTE_GAP, PALETTE_GATES, PALETTE_ROW_Y, PALETTE_SIZE, SLOT_COUNT, SLOT_LEFT, SLOT_SPACING, SLOT_RIGHT, SNAP_DISTANCE } from './constants'
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
  const lineYs = LINE_Y_VALUES

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
    return { center: nearest, distance: nearestDistance }
  }

  const getOccupiedSlots = (wireIndex: number, ignoreId: number | null) => {
    const occupied = new Set<number>()
    placedGates.forEach((gate) => {
      if (gate.dragging || gate.wire !== wireIndex || gate.id === ignoreId) {
        return
      }
      const centerX = gate.x + GATE_SIZE / 2
      const snapped = nearestSlotCenter(centerX)
      occupied.add(snapped.center)
    })
    return occupied
  }

  const nearestAvailableSlot = (x: number, wireIndex: number, ignoreId: number | null) => {
    const occupied = getOccupiedSlots(wireIndex, ignoreId)
    let nearest = slotCenters[0] ?? x
    let nearestDistance = Math.abs(x - nearest)
    let found = false
    for (const slot of slotCenters) {
      if (occupied.has(slot)) {
        continue
      }
      const distance = Math.abs(x - slot)
      if (!found || distance < nearestDistance) {
        nearest = slot
        nearestDistance = distance
        found = true
      }
    }
    return found ? { center: nearest, distance: nearestDistance } : null
  }

  const nearestLine = (y: number) => {
    let nearest = lineYs[0] ?? y
    let nearestDistance = Math.abs(y - nearest)
    let nearestIndex = 0
    lineYs.forEach((lineY, index) => {
      const distance = Math.abs(y - lineY)
      if (distance < nearestDistance) {
        nearest = lineY
        nearestDistance = distance
        nearestIndex = index
      }
    })
    return { y: nearest, distance: nearestDistance, index: nearestIndex }
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
          wire: 0,
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
    const centerY = nextY + GATE_SIZE / 2
    const line = nearestLine(centerY)
    const shouldSnapY = line.distance <= SNAP_DISTANCE
    if (shouldSnapY) {
      gate.y = line.y - GATE_SIZE / 2
      gate.wire = line.index
      const centerX = x - dragOffsetX + GATE_SIZE / 2
      const snapped = nearestAvailableSlot(centerX, line.index, gate.id)
      if (snapped) {
        gate.x = snapped.center - GATE_SIZE / 2
      }
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
    const line = nearestLine(centerY)
    const snapped = nearestAvailableSlot(centerX, line.index, gate.id)
    const onCircuit =
      centerX >= SLOT_LEFT &&
      centerX <= SLOT_RIGHT &&
      line.distance <= SNAP_DISTANCE &&
      snapped !== null &&
      snapped.distance <= SNAP_DISTANCE
    if (!onCircuit) {
      placedGates.splice(gateIndex, 1)
    } else {
      gate.x = snapped.center - GATE_SIZE / 2
      gate.y = line.y - GATE_SIZE / 2
      gate.wire = line.index
      onGateDropped(gate)
    }
    activeGateId = null
    onUpdate()
  }

  canvas.addEventListener('pointerup', handlePointerEnd)
  canvas.addEventListener('pointercancel', handlePointerEnd)
}
