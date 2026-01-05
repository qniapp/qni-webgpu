import assert from 'node:assert/strict'
import { test } from 'node:test'
import { createCircuit, placeGate, runCircuit, setQubits } from '../src/circuit.js'

const EPSILON = 1e-10

function approxEqual(actual, expected, message) {
  assert.ok(Math.abs(actual - expected) < EPSILON, message)
}

test('runCircuit applies H gate on single qubit', () => {
  const circuit = createCircuit(1)
  placeGate(circuit, { gate: 'H', target: 0, column: 0 })
  const state = runCircuit(circuit)
  const invSqrt2 = 1 / Math.sqrt(2)

  approxEqual(state[0].re, invSqrt2, 'state[0].re')
  approxEqual(state[0].im, 0, 'state[0].im')
  approxEqual(state[1].re, invSqrt2, 'state[1].re')
  approxEqual(state[1].im, 0, 'state[1].im')
})

test('runCircuit applies X on qubit 0 for two qubits', () => {
  const circuit = createCircuit(2)
  placeGate(circuit, { gate: 'X', target: 0, column: 0 })
  const state = runCircuit(circuit)

  assert.equal(state.length, 4)
  approxEqual(state[0].re, 0, 'state[0].re')
  approxEqual(state[1].re, 1, 'state[1].re')
  approxEqual(state[2].re, 0, 'state[2].re')
  approxEqual(state[3].re, 0, 'state[3].re')
})

test('runCircuit respects column order', () => {
  const circuit = createCircuit(1)
  placeGate(circuit, { gate: 'Z', target: 0, column: 1 })
  placeGate(circuit, { gate: 'H', target: 0, column: 0 })
  const state = runCircuit(circuit)
  const invSqrt2 = 1 / Math.sqrt(2)

  approxEqual(state[0].re, invSqrt2, 'state[0].re')
  approxEqual(state[1].re, -invSqrt2, 'state[1].re')
})

test('setQubits resets circuit operations', () => {
  const circuit = createCircuit(1)
  placeGate(circuit, { gate: 'X', target: 0, column: 0 })
  setQubits(circuit, 2)

  assert.equal(circuit.qubits, 2)
  assert.equal(circuit.operations.length, 0)
})
