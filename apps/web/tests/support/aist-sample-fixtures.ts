export const BELL_SAMPLE_JSON = JSON.stringify({ cols: [
  ['{superposition'], ['H'], ['Bloch'], ['}'], [1],
  ['{entangle'], ['•', 'X'], ['Amps2'], [1], ['}'],
] })

export const GHZ_SAMPLE_JSON = JSON.stringify({ cols: [
  ['{superposition'], ['H'], ['Bloch'], ['}'], [1],
  ['{entangle'], ['•', 'X'], ['•', 1, 'X'], ['Amps3'], [1], ['}'],
] })

const oracle = ['Z', '•', '◦', '•', '•']
const diffuser = [['H', 'H', 'H', 'H'], ['•', '•', '•', '•', 'X'], ['H', 'H', 'H', 'H'], ['Probability5']]
export const GROVER_SAMPLE_JSON = JSON.stringify({ cols: [
  ['{superposition'], ['X', 'X', 'X', 'X', 'X'], ['H', 'H', 'H', 'H', 'H'], ['Probability5'], ['}'], [1],
  ['{oracle'], oracle, ['}'], [1],
  ['{diffuser'], ...diffuser, ['}'], [1],
  ['{iteration 2'], oracle, ...diffuser, ['}'], [1],
  ['{iteration 3'], oracle, ...diffuser, ['}'], [1],
  ['{iteration 4'], oracle, ...diffuser, ['}'],
] })
