import type { Gate } from './gate'

export type Complex = {
  re: number
  im: number
}

const INV_SQRT2 = 1 / Math.sqrt(2)
const PHASE_45: Complex = { re: INV_SQRT2, im: INV_SQRT2 }

const SQRT_X: Complex = { re: 0.5, im: 0.5 }
const SQRT_X_INV: Complex = { re: 0.5, im: -0.5 }
const T_DAGGER: Complex = { re: INV_SQRT2, im: -INV_SQRT2 }

const GATE_MATRICES: Record<Gate, [Complex, Complex, Complex, Complex]> = {
  H: [
    { re: INV_SQRT2, im: 0 },
    { re: INV_SQRT2, im: 0 },
    { re: INV_SQRT2, im: 0 },
    { re: -INV_SQRT2, im: 0 },
  ],
  X: [
    { re: 0, im: 0 },
    { re: 1, im: 0 },
    { re: 1, im: 0 },
    { re: 0, im: 0 },
  ],
  Y: [
    { re: 0, im: 0 },
    { re: 0, im: -1 },
    { re: 0, im: 1 },
    { re: 0, im: 0 },
  ],
  Z: [
    { re: 1, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: 0 },
    { re: -1, im: 0 },
  ],
  '√X': [SQRT_X, SQRT_X_INV, SQRT_X_INV, SQRT_X],
  S: [
    { re: 1, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: 1 },
  ],
  'S†': [
    { re: 1, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: -1 },
  ],
  T: [
    { re: 1, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: 0 },
    PHASE_45,
  ],
  'T†': [
    { re: 1, im: 0 },
    { re: 0, im: 0 },
    { re: 0, im: 0 },
    T_DAGGER,
  ],
}

export function formatComplex(value: Complex): string {
  const re = Object.is(value.re, -0) ? 0 : value.re
  const im = Object.is(value.im, -0) ? 0 : value.im
  const sign = im < 0 ? '-' : '+'
  const absIm = Math.abs(im)
  return `${re}${sign}${absIm}i`
}

export function applyGateToZero(gate: Gate): [Complex, Complex] {
  const zero: Complex = { re: 1, im: 0 }
  const one: Complex = { re: 0, im: 0 }
  const [a00, a01, a10, a11] = GATE_MATRICES[gate]

  const mul = (a: Complex, b: Complex): Complex => ({
    re: a.re * b.re - a.im * b.im,
    im: a.re * b.im + a.im * b.re,
  })

  const add = (a: Complex, b: Complex): Complex => ({
    re: a.re + b.re,
    im: a.im + b.im,
  })

  const out0 = add(mul(a00, zero), mul(a01, one))
  const out1 = add(mul(a10, zero), mul(a11, one))
  return [out0, out1]
}
