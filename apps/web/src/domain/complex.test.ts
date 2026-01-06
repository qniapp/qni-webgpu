import { describe, expect, test } from 'vitest'
import { applyGateToZero, formatComplex } from './complex'

describe('formatComplex', () => {
  test('formats positive and negative values', () => {
    expect(formatComplex({ re: 1.5, im: 2 })).toBe('1.5+2i')
    expect(formatComplex({ re: -1, im: -3 })).toBe('-1-3i')
  })

  test('normalizes -0', () => {
    expect(formatComplex({ re: -0, im: 0 })).toBe('0+0i')
  })
})

describe('applyGateToZero', () => {
  test('X gate', () => {
    const [a0, a1] = applyGateToZero('X')
    expect(a0.re).toBe(0)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(1)
    expect(a1.im).toBe(0)
  })

  test('H gate', () => {
    const [a0, a1] = applyGateToZero('H')
    const inv = 1 / Math.sqrt(2)
    expect(a0.re).toBeCloseTo(inv)
    expect(a0.im).toBe(0)
    expect(a1.re).toBeCloseTo(inv)
    expect(a1.im).toBe(0)
  })

  test('Y gate', () => {
    const [a0, a1] = applyGateToZero('Y')
    expect(a0.re).toBe(0)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(0)
    expect(a1.im).toBe(1)
  })

  test('Z gate', () => {
    const [a0, a1] = applyGateToZero('Z')
    expect(a0.re).toBe(1)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(0)
    expect(a1.im).toBe(0)
  })

  test('S gate', () => {
    const [a0, a1] = applyGateToZero('S')
    expect(a0.re).toBe(1)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(0)
    expect(a1.im).toBe(0)
  })

  test('T gate', () => {
    const [a0, a1] = applyGateToZero('T')
    expect(a0.re).toBe(1)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(0)
    expect(a1.im).toBe(0)
  })

  test('√X gate', () => {
    const [a0, a1] = applyGateToZero('√X')
    expect(a0.re).toBeCloseTo(0.5, 6)
    expect(a0.im).toBeCloseTo(0.5, 6)
    expect(a1.re).toBeCloseTo(0.5, 6)
    expect(a1.im).toBeCloseTo(-0.5, 6)
  })

  test('S† gate', () => {
    const [a0, a1] = applyGateToZero('S†')
    expect(a0.re).toBe(1)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(0)
    expect(a1.im).toBe(0)
  })

  test('T† gate', () => {
    const [a0, a1] = applyGateToZero('T†')
    expect(a0.re).toBe(1)
    expect(a0.im).toBe(0)
    expect(a1.re).toBe(0)
    expect(a1.im).toBe(0)
  })
})
