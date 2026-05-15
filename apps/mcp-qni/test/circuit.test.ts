import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  createCircuit,
  placeGate,
  runCircuit,
  setQubits,
} from '../src/circuit.js'

const EPSILON = 1e-10

function approxEqual(actual: number, expected: number): boolean {
  return Math.abs(actual - expected) < EPSILON
}

test('runCircuit applies H gate on single qubit', () => {
  const circuit = createCircuit(1)
  placeGate(circuit, { gate: 'H', target: 0, column: 0 })
  const state = runCircuit(circuit)
  const invSqrt2 = 1 / Math.sqrt(2)

  assert.deepEqual(
    {
      state0Re: approxEqual(state[0].re, invSqrt2),
      state0Im: approxEqual(state[0].im, 0),
      state1Re: approxEqual(state[1].re, invSqrt2),
      state1Im: approxEqual(state[1].im, 0),
    },
    { state0Re: true, state0Im: true, state1Re: true, state1Im: true }
  )
})

test('runCircuit applies X on qubit 0 for two qubits', () => {
  const circuit = createCircuit(2)
  placeGate(circuit, { gate: 'X', target: 0, column: 0 })
  const state = runCircuit(circuit)

  assert.deepEqual(
    {
      length: state.length,
      state0Re: approxEqual(state[0].re, 0),
      state1Re: approxEqual(state[1].re, 1),
      state2Re: approxEqual(state[2].re, 0),
      state3Re: approxEqual(state[3].re, 0),
    },
    {
      length: 4,
      state0Re: true,
      state1Re: true,
      state2Re: true,
      state3Re: true,
    }
  )
})

test('runCircuit respects column order', () => {
  const circuit = createCircuit(1)
  placeGate(circuit, { gate: 'Z', target: 0, column: 1 })
  placeGate(circuit, { gate: 'H', target: 0, column: 0 })
  const state = runCircuit(circuit)
  const invSqrt2 = 1 / Math.sqrt(2)

  assert.deepEqual(
    {
      state0Re: approxEqual(state[0].re, invSqrt2),
      state1Re: approxEqual(state[1].re, -invSqrt2),
    },
    { state0Re: true, state1Re: true }
  )
})

test('setQubits resets circuit operations', () => {
  const circuit = createCircuit(1)
  placeGate(circuit, { gate: 'X', target: 0, column: 0 })
  setQubits(circuit, 2)

  assert.deepEqual(
    { qubits: circuit.qubits, operations: circuit.operations.length },
    { qubits: 2, operations: 0 }
  )
})
