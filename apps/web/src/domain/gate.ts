export type Gate = 'X' | 'H' | 'Y' | 'Z' | 'S' | 'T'

export const GATE_ORDER: Gate[] = ['X', 'H', 'Y', 'Z', 'S', 'T']
export const DEFAULT_GATE: Gate = 'H'

export function isGate(value: string): value is Gate {
  return GATE_ORDER.includes(value as Gate)
}

export function gateToIndex(gate: Gate): number {
  return GATE_ORDER.indexOf(gate)
}

export function getGateFromQuery(): Gate {
  const raw = new URLSearchParams(window.location.search).get('gate')
  if (!raw) {
    return DEFAULT_GATE
  }
  const normalized = raw.trim().toUpperCase()
  return isGate(normalized) ? normalized : DEFAULT_GATE
}
