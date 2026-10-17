"""ZDetectControlReset contracts and exact, dependency-free WebGPU fixtures.

Only the optional Aer class imports simulator dependencies. The exact reference
interprets emitted primitives, not the high-level gate under test.
"""
import math
import unittest

from qni_qiskit_backend.circuit import CircuitBuildError, apply_column_to_qiskit
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import QiskitRunner, RunnerUnavailable, load_qiskit
from test_axis_detectors import (
    FakeCircuit, MATRICES, build, exact_branches as axis_branches, single_gate, snapshot,
)

ZDCR = "ZDetectControlReset"
SEED = 8128
COMBINATIONS = (
    ((1, 1), "•", 1),
    ((1, 1), "•", 0),
    ((1, 0), "•", 1),
    ((0, 1), "•", 1),
    ((1, 1), "◦", 0),
    ((1, 1), "◦", 1),
)


class ZDetectControlResetFakeTests(unittest.TestCase):
    def test_measure_control_reset_order(self):
        self.assertEqual(build([[ZDCR, "X"]], 2), [
            ("measure", 0, 0), ("mcx", [0], 1), ("reset", 0),
        ])

    def test_empty_body_still_measures_and_resets_without_control_only_z(self):
        self.assertEqual(build([[ZDCR, ZDCR]], 2), [
            ("measure", 0, 0), ("measure", 1, 1), ("reset", 0), ("reset", 1),
        ])

    def test_controls_only_with_zdcr_never_emits_cz(self):
        self.assertEqual(build([[ZDCR, "•", "•"]], 3), [
            ("measure", 0, 0), ("reset", 0),
        ])

    def test_controlled_write_uses_sample_before_reset(self):
        self.assertEqual(build([["X"], [ZDCR, "|1>"]], 2), [
            ("x", 0), ("measure", 0, 0), ("mcx", [0], 1), ("reset", 0),
        ])

    def test_measurement_precedes_detector_basis_even_when_lower_wire(self):
        self.assertEqual(build([["YDetector", ZDCR]], 2), [
            ("measure", 1, 1), ("sdg", 0), ("h", 0),
            ("detector_projector", 0, [1], []), ("h", 0), ("s", 0), ("reset", 1),
        ])

    def test_multiple_measurements_precede_anti_control_flips(self):
        self.assertEqual(build([[ZDCR, ZDCR, "◦", "X"]], 4), [
            ("measure", 0, 0), ("measure", 1, 1), ("x", 2),
            ("mcx", [0, 1, 2], 3), ("x", 2), ("reset", 0), ("reset", 1),
        ])

    def test_measurement_is_unconditional_despite_unsatisfied_control(self):
        self.assertEqual(build([["•", ZDCR]], 2), [("measure", 1, 1), ("reset", 1)])

    def test_following_column_does_not_inherit_controls(self):
        self.assertEqual(build([[ZDCR, "X"], ["X", "X"]], 2), [
            ("measure", 0, 0), ("mcx", [0], 1), ("reset", 0), ("x", 0), ("x", 1),
        ])

    def test_swap_body_precedes_ordinary_measurement_and_reset(self):
        self.assertEqual(build([[ZDCR, "Swap", "Swap", "Measure"]], 4), [
            ("measure", 0, 0), ("mcx", [0, 1], 2), ("mcx", [0, 2], 1),
            ("mcx", [0, 1], 2), ("measure", 3, 3), ("reset", 0),
        ])

    def test_callback_after_inverse_basis_before_reset(self):
        circuit = FakeCircuit()
        basis = [1, 0]
        apply_column_to_qiskit(circuit, [ZDCR, "YDetector"], 2, basis,
                               save_displays=lambda: circuit.append(("display", tuple(basis))))
        self.assertEqual(circuit.ops, [
            ("measure", 0, 0), ("sdg", 1), ("h", 1),
            ("detector_projector", 1, [0], []), ("h", 1), ("s", 1),
            ("display", (1, None)), ("reset", 0),
        ])

    def test_full_column_webgpu_operation_order(self):
        circuit = FakeCircuit()
        apply_column_to_qiskit(
            circuit, [ZDCR, "H", "YDetector", "XDetector", "Swap", "Swap", "Measure", "Probability"],
            8, [None] + [0] * 7, save_displays=lambda: circuit.append(("display",)),
        )
        self.assertEqual(circuit.ops, [
            ("measure", 0, 0), ("sdg", 2), ("h", 2), ("h", 3),
            ("mcx", [0, 4], 5), ("mcx", [0, 5], 4), ("mcx", [0, 4], 5),
            ("controlled", ("H", None), [0], [1]),
            ("detector_projector", 2, [0], []), ("detector_projector", 3, [0], []),
            ("measure", 6, 6), ("h", 3), ("h", 2), ("s", 2),
            ("display",), ("reset", 0),
        ])

    def test_callback_on_column_without_reset(self):
        circuit = FakeCircuit()
        apply_column_to_qiskit(circuit, ["X"], 1, [0],
                               save_displays=lambda: circuit.append(("display",)))
        self.assertEqual(circuit.ops, [("x", 0), ("display",)])

    def test_callback_is_keyword_only(self):
        with self.assertRaises(TypeError):
            apply_column_to_qiskit(FakeCircuit(), [ZDCR], 1, [0], lambda: None)

    def test_out_of_range(self):
        with self.assertRaises(CircuitBuildError):
            build([[1, ZDCR]], 1)

    def test_last_valid_wire(self):
        self.assertEqual(build([[1, ZDCR]], 2), [("measure", 1, 1), ("reset", 1)])

    def test_zdcr_is_not_subject_to_projector_eight_wire_limit(self):
        self.assertEqual(build([[ZDCR] * 9], 9),
                         [("measure", w, w) for w in range(9)] + [("reset", w) for w in range(9)])

    def test_axis_projector_limit_includes_zdcr(self):
        with self.assertRaises(CircuitBuildError):
            build([[ZDCR] + ["•"] * 7 + ["XDetector"]], 9)

    def test_axis_projector_limit_includes_anti_controls(self):
        with self.assertRaises(CircuitBuildError):
            build([[ZDCR] + ["◦"] * 7 + ["YDetector"]], 9)

    def test_eight_wire_axis_projector_allowed(self):
        self.assertEqual(build([[ZDCR] + ["•"] * 6 + ["XDetector"]], 8), [
            ("measure", 0, 0), ("h", 7),
            ("detector_projector", 7, list(range(7)), []),
            ("h", 7), ("reset", 0),
        ])

    def test_qft_span_ends_immediately_before_zdcr(self):
        self.assertEqual(build([["QFT1", ZDCR]], 2), [
            ("measure", 1, 1), ("controlled", ("H", None), [1], [0]), ("reset", 1),
        ])

    def test_disjoint_qft_allowed(self):
        self.assertEqual(build([[ZDCR, "QFT1"]], 2), [
            ("measure", 0, 0), ("controlled", ("H", None), [0], [1]), ("reset", 0),
        ])


# Reset must use the pre-reset measured value for body basis tracking, then set 0.
def basis_test(initial, column, expected):
    def test(self):
        basis = initial[:]
        apply_column_to_qiskit(FakeCircuit(), column, len(basis), basis)
        self.assertEqual(basis, expected)
    return test


for _name, _initial, _column, _expected in (
    ("zero", [0, 0], [ZDCR, "X"], [0, 0]),
    ("one", [1, 0], [ZDCR, "X"], [0, 1]),
    ("unknown", [None, 0], [ZDCR, "X"], [0, None]),
    ("blocked", [None, 0, 1], [ZDCR, "•", "X"], [0, 0, 1]),
    ("anti_satisfied", [1, 0, 0], [ZDCR, "◦", "X"], [0, 0, 1]),
    ("anti_blocked", [1, 1, 0], [ZDCR, "◦", "X"], [0, 1, 0]),
    ("multiple", [1, 1, 0], [ZDCR, ZDCR, "X"], [0, 0, 1]),
    ("multiple_blocked", [None, 0, 1], [ZDCR, ZDCR, "X"], [0, 0, 1]),
):
    setattr(ZDetectControlResetFakeTests, "test_basis_" + _name,
            basis_test(_initial, _column, _expected))


def qft_overlap_test(token, column):
    def test(self):
        with self.assertRaises(CircuitBuildError):
            build([[token] + column], 4)
    return test


for _token in ("QFT2", "QFT†2", "QFT3", "QFT†3"):
    setattr(ZDetectControlResetFakeTests, "test_overlap_" + _token,
            qft_overlap_test(_token, [ZDCR, 1, 1]))


def exact_branches(ops, qubits):
    """Extend axis reference locally with controlled gates and physical reset.

    Axis helpers supply single-qubit gates and joint detector projection. A reset
    branches by Z outcome and flips only outcome 1; it never merges amplitudes.
    Reset's internal sample is deliberately absent from detector outcome history.
    """
    branches = axis_branches([], qubits)
    for op in ops:
        name = op[0]
        if name in {"reset", "measure", "detector_projector"}:
            updated = []
            target = op[1]
            controls, antis = (op[2:4] if name == "detector_projector" else ([], []))
            for weight, history, state in branches:
                for outcome in (0, 1):
                    projected = [a if int(bool(i & (1 << target))
                                             and all(i & (1 << c) for c in controls)
                                             and all(not i & (1 << c) for c in antis)) == outcome else 0j
                                 for i, a in enumerate(state)]
                    probability = sum(abs(a) ** 2 for a in projected)
                    if probability > 1e-12:
                        normalized = [a / math.sqrt(probability) for a in projected]
                        if name == "reset" and outcome:
                            normalized = single_gate(normalized, target, MATRICES["x"])
                        updated.append((weight * probability,
                                        history if name == "reset" else history + (outcome,), normalized))
            branches = updated
            continue
        updated = []
        for weight, history, state in branches:
            if name in MATRICES:
                result = single_gate(state, op[1], MATRICES[name])
            elif name in {"mcx", "cx", "controlled"}:
                if name == "controlled":
                    gate, controls, targets = op[1:]
                    matrix = MATRICES[{"S†": "sdg"}.get(gate[0], gate[0].lower())]
                    target = targets[0]
                else:
                    controls = op[1] if name == "mcx" else [op[1]]
                    target, matrix = op[2], MATRICES["x"]
                transformed = single_gate(state, target, matrix)
                result = [transformed[i] if all(i & (1 << c) for c in controls) else a
                          for i, a in enumerate(state)]
            elif name == "swap":
                first, second = op[1:]
                result = [0j] * len(state)
                for i, amplitude in enumerate(state):
                    mask = (1 << first | 1 << second) if ((i >> first) ^ (i >> second)) & 1 else 0
                    result[i ^ mask] = amplitude
            else:
                raise ValueError(f"reference interpreter does not support {op!r}")
            updated.append((weight, history, result))
        branches = updated
    return branches


def probabilities(columns, qubits):
    branches = exact_branches(build(columns, qubits), qubits)
    return [round(sum(p * abs(state[i]) ** 2 for p, _, state in branches), 8)
            for i in range(1 << qubits)]


def basis_probabilities(qubits, index):
    return [int(i == index) for i in range(1 << qubits)]


def prepare(bits):
    return ["X" if bit else "|0>" for bit in bits]


# WebGPU parity fixtures: apps/web/tests/z-detect-control-reset.spec.ts.
class ZDetectControlResetWebGpuParityTests(unittest.TestCase):
    def test_h_sample_correlates_with_target_and_resets(self):
        self.assertEqual(snapshot(exact_branches(build([["H"], [ZDCR, "X"]], 2), 2)), [
            (0.5, (0,), [1, 0, 0, 0, 0, 0, 0, 0]),
            (0.5, (1,), [0, 0, 0, 0, 1, 0, 0, 0]),
        ])

    def test_superposition_decomposition(self):
        # WebGPU Write0 after a Z sample equals physical reset. Compile that
        # final primitive locally: backend Write0 rejects an unknown basis.
        prefix = [["H"], ["T"]]
        self.assertEqual(snapshot(exact_branches(build(prefix + [[ZDCR, "X"]], 2), 2)),
                         snapshot(exact_branches(build(prefix + [["•", "X"], ["Measure"]], 2)
                                                 + [("reset", 0)], 2)))

    def test_two_independent_samples(self):
        self.assertEqual([(round(p, 8), history,
                           [round(abs(a) ** 2, 8) for a in state])
                          for p, history, state in exact_branches(
                              build([["H", "H"], [ZDCR, ZDCR, "X"]], 3), 3)],
                         [(0.25, (a, b), basis_probabilities(3, 4 * (a & b)))
                          for a in (0, 1) for b in (0, 1)])

    def test_following_column_knows_reset_zero(self):
        self.assertEqual(probabilities([["H"], [ZDCR], ["|1>"]], 1), [0, 1])


def deterministic_fixture(input_bit, following=False):
    columns = [prepare([input_bit]), [ZDCR, "X"]]
    if following:
        return columns + [["X", "X"]], 2, 1 + 2 * (1 - input_bit), None
    return columns, 2, 2 * input_bit, [prepare([input_bit]), ["•", "X"], ["Measure"], ["|0>"]]


def combination_fixture(inputs, control, value):
    prefix = prepare([*inputs, value])
    enabled = all(inputs) and value == (1 if control == "•" else 0)
    return ([prefix, [ZDCR, ZDCR, control, "X"]], 4, 4 * value + 8 * enabled,
            [prefix, ["•", "•", control, "X"], ["Measure", "Measure"], ["|0>", "|0>"]])


FIXTURES = [(f"input_{bit}" + ("_following" if following else ""), deterministic_fixture(bit, following))
            for bit in (0, 1) for following in (False, True)]
FIXTURES += [(f"combination_{i}", combination_fixture(*case)) for i, case in enumerate(COMBINATIONS)]


def exact_fixture_test(fixture):
    def test(self):
        columns, qubits, index, decomposition = fixture
        actual = [probabilities(columns, qubits)]
        if decomposition is not None:
            actual.append(probabilities(decomposition, qubits))
        self.assertEqual(actual, [basis_probabilities(qubits, index)] * len(actual))
    return test


for _name, _fixture in FIXTURES:
    setattr(ZDetectControlResetWebGpuParityTests, "test_webgpu_" + _name, exact_fixture_test(_fixture))


class ZDetectControlResetAerTests(unittest.TestCase):
    def setUp(self):
        try:
            load_qiskit()
        except RunnerUnavailable as exc:
            self.skipTest(str(exc))

    def run_columns(self, columns, qubits, outputs=None):
        raw = {"qubits": qubits, "columns": columns, "shots": 1024, "seed": SEED}
        if outputs is not None:
            raw["outputs"] = outputs
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(parse_run_request(raw))

    def test_h_sample_histogram(self):
        histogram = self.run_columns([["H"], [ZDCR, "X"]], 2)["histogram"]
        self.assertEqual((set(histogram), sum(histogram.values()),
                          abs(histogram.get("10", 0) / 1024 - 0.5) <= 0.05),
                         ({"00", "10"}, 1024, True))

    def test_display_before_reset(self):
        response = self.run_columns([["H"], [ZDCR, "Probability"]], 2, {
            "histogram": True,
            "probability": [{"gate_id": 7, "column": 1, "span": 1, "base_bit": 1,
                             "control_mask": 1, "control_value": 1}],
        })
        self.assertEqual(([round(p, 8) for p in response["probability"][0]["probabilities"]],
                          response["histogram"]), ([1, 0], {"00": 1024}))

    def test_conditional_bloch_before_reset(self):
        response = self.run_columns([["X", "H"], [ZDCR, "Bloch"]], 2, {
            "histogram": True,
            "bloch": [{"gate_id": 9, "column": 1, "wire": 1,
                       "control_mask": 1, "control_value": 1}],
        })
        self.assertEqual([round(p, 8) for p in response["bloch"][0]["vector"]], [1, 0, 0])

    def test_conditional_display_uses_measured_control_before_reset(self):
        response = self.run_columns([["X", "X"], [ZDCR, "Probability"]], 2, {
            "histogram": True,
            "probability": [{"gate_id": 8, "column": 1, "span": 1, "base_bit": 1,
                             "control_mask": 1, "control_value": 1}],
        })
        self.assertEqual(([round(p, 8) for p in response["probability"][0]["probabilities"]],
                          response["histogram"]), ([0, 1], {"10": 1024}))


def aer_fixture_test(fixture):
    def test(self):
        columns, qubits, index, decomposition = fixture
        actual = [self.run_columns(columns, qubits)["histogram"]]
        if decomposition is not None:
            actual.append(self.run_columns(decomposition, qubits)["histogram"])
        self.assertEqual(actual, [{format(index, f"0{qubits}b"): 1024}] * len(actual))
    return test


for _name, _fixture in FIXTURES:
    setattr(ZDetectControlResetAerTests, "test_webgpu_" + _name, aer_fixture_test(_fixture))


if __name__ == "__main__":
    unittest.main()
