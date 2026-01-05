// @vitest-environment jsdom
import { describe, expect, test } from 'vitest'
import { DEFAULT_GATE, GATE_ORDER, gateToIndex, getGateFromQuery } from './gate'

describe('getGateFromQuery', () => {
  test('uses default when missing', () => {
    window.history.pushState({}, '', '/')
    expect(getGateFromQuery()).toBe(DEFAULT_GATE)
  })

  test('parses lowercase gate', () => {
    window.history.pushState({}, '', '/?gate=y')
    expect(getGateFromQuery()).toBe('Y')
  })

  test('falls back on invalid value', () => {
    window.history.pushState({}, '', '/?gate=foo')
    expect(getGateFromQuery()).toBe(DEFAULT_GATE)
  })
})

describe('gateToIndex', () => {
  test('matches order', () => {
    GATE_ORDER.forEach((gate, index) => {
      expect(gateToIndex(gate)).toBe(index)
    })
  })
})
