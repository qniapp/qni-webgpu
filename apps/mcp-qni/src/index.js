import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import { CallToolRequestSchema, ListToolsRequestSchema } from '@modelcontextprotocol/sdk/types.js'
import { clearCircuit, createCircuit, placeGate, runCircuit, setQubits } from './circuit.js'

const server = new Server(
  {
    name: 'qni',
    version: '0.1.0',
  },
  {
    capabilities: {
      tools: {},
    },
  }
)

const circuit = createCircuit(1)

function toolError(message) {
  return {
    content: [{ type: 'text', text: message }],
    isError: true,
  }
}

function serializeCircuit() {
  return {
    qubits: circuit.qubits,
    operations: circuit.operations.map((operation) => ({
      gate: operation.gate,
      target: operation.target,
      column: operation.column,
    })),
  }
}

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: 'qni_set_qubits',
      description: 'Set the number of qubits and reset the circuit.',
      inputSchema: {
        type: 'object',
        properties: {
          qubits: { type: 'integer', minimum: 1 },
        },
        required: ['qubits'],
      },
    },
    {
      name: 'qni_place_gate',
      description: 'Place a single-qubit gate at the given qubit and column.',
      inputSchema: {
        type: 'object',
        properties: {
          gate: { type: 'string', enum: ['X', 'H', 'Y', 'Z', 'S', 'T'] },
          target: { type: 'integer', minimum: 0 },
          column: { type: 'integer', minimum: 0 },
        },
        required: ['gate', 'target', 'column'],
      },
    },
    {
      name: 'qni_clear_circuit',
      description: 'Remove all operations from the circuit.',
      inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    },
    {
      name: 'qni_get_circuit',
      description: 'Get the current circuit definition.',
      inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    },
    {
      name: 'qni_run',
      description: 'Execute the circuit and return the state vector.',
      inputSchema: { type: 'object', properties: {}, additionalProperties: false },
    },
  ],
}))

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args } = request.params
  try {
    switch (name) {
      case 'qni_set_qubits': {
        setQubits(circuit, Number(args?.qubits))
        return {
          content: [{ type: 'text', text: JSON.stringify(serializeCircuit()) }],
        }
      }
      case 'qni_place_gate': {
        placeGate(circuit, {
          gate: args?.gate,
          target: Number(args?.target),
          column: Number(args?.column),
        })
        return {
          content: [{ type: 'text', text: JSON.stringify(serializeCircuit()) }],
        }
      }
      case 'qni_clear_circuit': {
        clearCircuit(circuit)
        return {
          content: [{ type: 'text', text: JSON.stringify(serializeCircuit()) }],
        }
      }
      case 'qni_get_circuit': {
        return {
          content: [{ type: 'text', text: JSON.stringify(serializeCircuit()) }],
        }
      }
      case 'qni_run': {
        const stateVector = runCircuit(circuit)
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify({
                qubits: circuit.qubits,
                stateVector: stateVector.map((entry) => [entry.re, entry.im]),
              }),
            },
          ],
        }
      }
      default:
        return toolError(`Unknown tool: ${name}`)
    }
  } catch (error) {
    const message =
      error && typeof error.message === 'string' ? error.message : String(error)
    return toolError(message)
  }
})

const transport = new StdioServerTransport()
await server.connect(transport)
