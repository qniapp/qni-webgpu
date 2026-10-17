"""Axis detector contracts, with a dependency-free exact branch reference."""
import cmath
import math
import unittest

from qni_qiskit_backend.circuit import (
    CircuitBuildError,
    apply_column_to_qiskit,
    apply_columns_to_qiskit,
)
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import QiskitRunner, RunnerUnavailable, load_qiskit


class FakeCircuit:
    def __init__(self):
        self.ops = []

    def __getattr__(self, name):
        if name not in {"h", "s", "sdg", "x", "z", "t", "swap", "measure", "reset", "cx", "mcx", "cp"}:
            raise AttributeError(name)
        return lambda *args: self.ops.append((name, *args))

    def append(self, instruction, *args):
        self.ops.append(instruction if not args else ("append", instruction, *args))


def build(columns, qubits=1):
    circuit = FakeCircuit()
    apply_columns_to_qiskit(circuit, columns, qubits)
    return circuit.ops


class AxisDetectorFakeTests(unittest.TestCase):
    def test_x_operation_order(self):
        self.assertEqual(build([["XDetector"]]), [("h", 0), ("measure", 0, 0), ("h", 0)])

    def test_y_operation_order(self):
        self.assertEqual(build([["YDetector"]]), [
            ("sdg", 0), ("h", 0), ("measure", 0, 0), ("h", 0), ("s", 0),
        ])

    def test_basis_before_swaps_and_gates_measurements_after_in_wire_order(self):
        self.assertEqual(build([["Measure", "YDetector", "Swap", "Swap", "X", "XDetector"]], 6), [
            ("sdg", 1), ("h", 1), ("h", 5), ("swap", 2, 3), ("x", 4),
            ("measure", 0, 0), ("measure", 1, 1), ("measure", 5, 5),
            ("h", 5), ("h", 1), ("s", 1),
        ])

    def test_detector_wraps_existing_qft_and_column_gate_order(self):
        self.assertEqual(build([["XDetector", "QFT1", "X"]], 3), [
            ("h", 0), ("h", 1), ("x", 2), ("measure", 0, 0), ("h", 0),
        ])

    def test_controlled_projector_tuple_and_no_control_only_cz(self):
        self.assertEqual(build([["•", "◦", "•", "YDetector"]], 4), [
            ("sdg", 3), ("h", 3), ("detector_projector", 3, [0, 2], [1]),
            ("h", 3), ("s", 3),
        ])

    def test_each_controlled_detector_has_its_own_projector(self):
        self.assertEqual(build([["•", "XDetector", "YDetector"]], 3), [
            ("h", 1), ("sdg", 2), ("h", 2),
            ("detector_projector", 1, [0], []),
            ("detector_projector", 2, [0], []),
            ("h", 2), ("s", 2), ("h", 1),
        ])

    def test_projector_allows_eight_total_wires(self):
        self.assertEqual(build([["•"] * 7 + ["XDetector"]], 8), [
            ("h", 7), ("detector_projector", 7, list(range(7)), []), ("h", 7),
        ])

    def test_projector_rejects_more_than_eight_total_wires(self):
        with self.assertRaises(CircuitBuildError):
            build([["•"] * 8 + ["XDetector"]], 9)

    def test_projector_limit_counts_anti_controls(self):
        with self.assertRaises(CircuitBuildError):
            build([["•"] * 4 + ["◦"] * 4 + ["YDetector"]], 9)

    def test_detector_rejects_wire_beyond_register(self):
        with self.assertRaises(CircuitBuildError):
            build([[1, "XDetector"]], 1)


def basis_case(axis, controls, initial, expected):
    def test(self):
        basis = initial[:]
        apply_column_to_qiskit(FakeCircuit(), controls + [axis + "Detector"], len(basis), basis)
        self.assertEqual(basis, expected)
    return test


for _axis in ("X", "Y"):
    for _name, _controls, _initial, _expected in (
        ("uncontrolled", [], [0], [None]),
        ("satisfied_positive", ["•"], [1, 0], [1, None]),
        ("satisfied_negative", ["◦"], [0, 1], [0, None]),
        ("unknown_positive", ["•"], [None, 0], [None, None]),
        ("unknown_negative", ["◦"], [None, 1], [None, None]),
        ("unsatisfied_positive", ["•"], [0, 1], [0, 1]),
        ("unsatisfied_negative", ["◦"], [1, 0], [1, 0]),
        ("unsatisfied_beats_unknown", ["•", "◦"], [0, None, 1], [0, None, 1]),
    ):
        setattr(AxisDetectorFakeTests, f"test_{_axis.lower()}_basis_{_name}",
                basis_case(_axis, _controls, _initial, _expected))


def single_gate(state, wire, matrix):
    """Little-endian statevector, without importing numpy or Qiskit."""
    result = state[:]
    bit = 1 << wire
    for index in range(len(state)):
        if not index & bit:
            a, b = state[index], state[index | bit]
            result[index] = matrix[0][0] * a + matrix[0][1] * b
            result[index | bit] = matrix[1][0] * a + matrix[1][1] * b
    return result


ROOT_HALF = math.sqrt(0.5)
MATRICES = {
    "h": ((ROOT_HALF, ROOT_HALF), (ROOT_HALF, -ROOT_HALF)),
    "s": ((1, 0), (0, 1j)),
    "sdg": ((1, 0), (0, -1j)),
    "x": ((0, 1), (1, 0)),
    "z": ((1, 0), (0, -1)),
    "t": ((1, 0), (0, cmath.exp(1j * math.pi / 4))),
}


def exact_branches(ops, qubits):
    """Interpret emitted primitive operations; projector's no-click is I-P.

    Each result is (probability, outcome history, normalized pure state).
    In particular no-click must NOT measure the controls separately.
    """
    branches = [(1.0, (), [1 + 0j] + [0j] * ((1 << qubits) - 1))]
    for op in ops:
        updated = []
        for weight, outcomes, state in branches:
            name = op[0]
            if name in MATRICES:
                updated.append((weight, outcomes, single_gate(state, op[1], MATRICES[name])))
            elif name == "swap":
                first, second = op[1:]
                result = [0j] * len(state)
                for index, amplitude in enumerate(state):
                    different = ((index >> first) ^ (index >> second)) & 1
                    result[index ^ ((1 << first | 1 << second) if different else 0)] = amplitude
                updated.append((weight, outcomes, result))
            elif name in {"measure", "detector_projector"}:
                target = op[1]
                controls, antis = (op[2], op[3]) if name == "detector_projector" else ([], [])
                for outcome in (0, 1):
                    projected = [
                        amplitude if int(bool(index & (1 << target))
                                         and all(index & (1 << c) for c in controls)
                                         and all(not index & (1 << c) for c in antis)) == outcome else 0j
                        for index, amplitude in enumerate(state)
                    ]
                    probability = sum(abs(a) ** 2 for a in projected)
                    if probability > 1e-12:
                        updated.append((weight * probability, outcomes + (outcome,),
                                        [a / math.sqrt(probability) for a in projected]))
            else:
                raise ValueError(f"reference interpreter does not support {op!r}")
        branches = updated
    return branches


def flat(state):
    # Match the WebGPU spec's interleaved real/imaginary arrays, including phase.
    return [round(part, 4) if abs(part) > 0.00005 else 0
            for value in state for part in (complex(value).real, complex(value).imag)]


def snapshot(branches):
    return [(round(weight, 8), outcomes, flat(state)) for weight, outcomes, state in branches]


def eigenstate(axis, outcome):
    return [ROOT_HALF, (-1 if outcome else 1) * ROOT_HALF * (1j if axis == "Y" else 1)]


def joint_expected(axis, control, outcome):
    # Literal WebGPU fixtures: wire 0 is control, wire 1 is detector.
    imaginary = axis == "Y"
    if outcome:
        return ([0, 0, 0.7071, 0, 0, 0, 0, -0.7071] if imaginary else
                [0, 0, 0.7071, 0, 0, 0, -0.7071, 0]) if control == "•" else (
                    [0.7071, 0, 0, 0, 0, -0.7071, 0, 0] if imaginary else
                    [0.7071, 0, 0, 0, -0.7071, 0, 0, 0])
    return ([0.8165, 0, 0.4082, 0, 0, 0, 0, 0.4082] if imaginary else
            [0.8165, 0, 0.4082, 0, 0, 0, 0.4082, 0]) if control == "•" else (
                [0.4082, 0, 0.8165, 0, 0, 0.4082, 0, 0] if imaginary else
                [0.4082, 0, 0.8165, 0, 0.4082, 0, 0, 0])


# WebGPU parity fixtures: apps/web/tests/web-detector.spec.ts.
class AxisDetectorWebGpuParityTests(unittest.TestCase):
    pass


def eigen_test(axis, outcome):
    def test(self):
        preparation = [["H"]] + ([["Z"]] if outcome else []) + ([["S"]] if axis == "Y" else [])
        self.assertEqual(snapshot(exact_branches(build(preparation + [[axis + "Detector"]]), 1)),
                         [(1.0, (outcome,), flat(eigenstate(axis, outcome)))])
    return test


def zero_test(axis):
    def test(self):
        self.assertEqual(snapshot(exact_branches(build([[axis + "Detector"]]), 1)),
                         [(0.5, (outcome,), flat(eigenstate(axis, outcome))) for outcome in (0, 1)])
    return test


def joint_test(axis, control, outcome):
    def test(self):
        branches = exact_branches(build([["H"], [control, axis + "Detector"]], 2), 2)
        self.assertEqual([(round(p, 8), flat(state)) for p, history, state in branches if history == (outcome,)],
                         [(0.25 if outcome else 0.75, joint_expected(axis, control, outcome))])
    return test


def decomposition_test(axis):
    def test(self):
        prefix = [["H"], ["T"]]
        before = [["H"]] if axis == "X" else [["S†"], ["H"]]
        after = [["H"]] if axis == "X" else [["H"], ["S"]]
        self.assertEqual(snapshot(exact_branches(build(prefix + [[axis + "Detector"]]), 1)),
                         snapshot(exact_branches(build(prefix + before + [["Measure"]] + after), 1)))
    return test


for _axis in ("X", "Y"):
    setattr(AxisDetectorWebGpuParityTests, f"test_{_axis}_zero_half_click_and_branches", zero_test(_axis))
    setattr(AxisDetectorWebGpuParityTests, f"test_{_axis}_explicit_decomposition", decomposition_test(_axis))
    for _outcome in (0, 1):
        setattr(AxisDetectorWebGpuParityTests, f"test_{_axis}_eigenstate_{_outcome}", eigen_test(_axis, _outcome))
        for _control, _label in (("•", "positive"), ("◦", "negative")):
            setattr(AxisDetectorWebGpuParityTests, f"test_{_axis}_{_label}_branch_{_outcome}",
                    joint_test(_axis, _control, _outcome))


class AxisDetectorAerTests(unittest.TestCase):
    def setUp(self):
        try:
            self.QuantumCircuit, self.transpile, _, self.AerSimulator = load_qiskit()
        except RunnerUnavailable as exc:
            self.skipTest(str(exc))

    def sample_branches(self, axis, controls):
        qubits = len(controls) + 1
        # Superpose every control: a wrong endian ordering changes the clicked subspace.
        columns = [["H"] * len(controls)] if controls else []
        columns += [controls + [axis + "Detector"]]
        expected = exact_branches(build(columns, qubits), qubits)
        circuit = self.QuantumCircuit(qubits, qubits)
        apply_columns_to_qiskit(circuit, columns, qubits)
        circuit.save_statevector(label="branches", pershot=True)
        simulator = self.AerSimulator(method="statevector", max_parallel_threads=1)
        result = simulator.run(self.transpile(circuit, simulator), shots=1024, seed_simulator=8128).result()
        states = result.data(0)["branches"]
        counts = [0] * len(expected)
        bad = 0
        for saved in states:
            state = list(saved.data)
            fidelities = [abs(sum(complex(a).conjugate() * complex(b) for a, b in zip(branch, state))) ** 2
                          for _, _, branch in expected]
            best = max(range(len(expected)), key=fidelities.__getitem__)
            counts[best] += 1
            bad += int(abs(fidelities[best] - 1) > 1e-8)
        return len(states), bad, counts, [p for p, _, _ in expected]

    def test_runner_probability_after_x_detector(self):
        request = parse_run_request({
            "qubits": 1, "columns": [["XDetector"], ["Probability"]], "shots": 256, "seed": 8128,
            "outputs": {"histogram": True, "probability": [
                {"gate_id": 7, "column": 1, "span": 1, "base_bit": 0},
            ]},
        })
        response = QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(request)
        self.assertEqual([round(p, 8) for p in response["probability"][0]["probabilities"]], [0.5, 0.5])

    def detector_histogram(self):
        request = parse_run_request({"qubits": 1, "columns": [["YDetector"]], "shots": 256, "seed": 8128})
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(request)["histogram"]

    def test_runner_histogram_support(self):
        self.assertEqual(set(self.detector_histogram()), {"0", "1"})

    def test_runner_histogram_shot_count(self):
        self.assertEqual(sum(self.detector_histogram().values()), 256)

    def test_runner_histogram_final_z_frequency(self):
        self.assertLessEqual(abs(self.detector_histogram()["1"] / 256 - 0.5), 0.1)


def aer_test(axis, controls, check):
    def test(self):
        count, bad, counts, probabilities = self.sample_branches(axis, controls)
        if check == "states":
            error, tolerance = bad, 0
        elif check == "shot_count":
            error, tolerance = abs(count - 1024), 0
        else:
            error = max(abs(n / 1024 - p) for n, p in zip(counts, probabilities))
            tolerance = 0.05
        self.assertLessEqual(error, tolerance, (count, bad, counts, probabilities))
    return test


for _axis in ("X", "Y"):
    for _label, _controls in (("uncontrolled", []), ("positive", ["•"]),
                              ("negative", ["◦"]), ("mixed", ["•", "◦", "•"])):
        for _check in ("states", "shot_count", "frequency"):
            setattr(AxisDetectorAerTests, f"test_{_axis}_{_label}_{_check}",
                    aer_test(_axis, _controls, _check))


if __name__ == "__main__":
    unittest.main()
