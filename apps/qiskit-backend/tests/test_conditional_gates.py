import cmath
from contextlib import contextmanager
import math
import unittest

from qni_qiskit_backend import circuit as circuit_module
from qni_qiskit_backend.circuit import (
    CircuitBuildError, apply_column_to_qiskit, apply_columns_to_qiskit,
)
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import (
    QiskitRunner, RunnerUnavailable, add_display_saves, load_qiskit,
)
from test_axis_detectors import FakeCircuit as AxisFakeCircuit, MATRICES, single_gate


class FakeCircuit(AxisFakeCircuit):
    def __getattr__(self, name):
        if name in {"y", "sx", "tdg", "p", "rx", "ry", "rz"}:
            return lambda *args: self.ops.append((name, *args))
        return super().__getattr__(name)

    @contextmanager
    def if_test(self, condition):
        self.ops.append(("if", *condition))
        yield
        self.ops.append(("end_if",))

    def save_probabilities(self, qargs, *, label):
        self.ops.append(("probability", qargs, label))


def build(columns, qubits=1):
    qc = FakeCircuit()
    apply_columns_to_qiskit(qc, columns, qubits)
    return qc.ops


WRITE = [("measure", 0, 0), ("measure", 0, "flag_0")]
IFABLE = ("H", "X", "Y", "Z", "√X", "S", "S†", "T", "T†")


class ConditionalFakeTests(unittest.TestCase):
    def test_plain_measure_unchanged(self):
        self.assertEqual(build([["Measure"]]), [("measure", 0, 0)])

    def test_named_measure_double_write_no_register_operation(self):
        self.assertEqual(build([["Measure>a"]]), WRITE)

    def test_trimmed_name_persists_across_empty_columns(self):
        self.assertEqual(build([["Measure> a "], [], ["X<\ta\n"]]),
                         WRITE + [("if", "flag_0", 1), ("x", 0), ("end_if",)])

    def test_empty_names_are_plain_operations(self):
        self.assertEqual(build([["Measure> "], ["X<\t"], ["Measure>"], ["H<"]]),
                         [("measure", 0, 0), ("x", 0), ("measure", 0, 0), ("h", 0)])

    def test_unset_gate_does_not_turn_controls_into_control_only_z(self):
        self.assertEqual(build([["•", "•", "X<missing"]], 3), [])

    def test_same_column_write_not_visible_to_unitaries(self):
        self.assertEqual(build([["Measure>a", "X<a"]], 2), WRITE)

    def test_same_column_overwrite_uses_old_source(self):
        self.assertEqual(build([["Measure>a"], ["Measure>a", "X<a"]], 2), WRITE + [
            ("if", "flag_0", 1), ("x", 1), ("end_if",),
            ("measure", 0, 0), ("measure", 0, "flag_1"),
        ])

    def test_bottommost_write_wins(self):
        self.assertEqual(build([["Measure>a", "Measure>a"], ["X<a"]], 2), WRITE + [
            ("measure", 1, 1), ("measure", 1, "flag_1"),
            ("if", "flag_1", 1), ("x", 0), ("end_if",),
        ])

    def test_flags_are_independent_and_each_write_gets_new_bit(self):
        self.assertEqual(build([["Measure>a"], ["Measure>b"], ["Measure>a"],
                                ["X<a", "Z<b"]], 2), WRITE + [
            ("measure", 0, 0), ("measure", 0, "flag_1"),
            ("measure", 0, 0), ("measure", 0, "flag_2"),
            ("if", "flag_2", 1), ("x", 0), ("end_if",),
            ("if", "flag_1", 1), ("z", 1), ("end_if",),
        ])

    def test_unitaries_before_measurements_in_wire_order(self):
        self.assertEqual(build([["Measure>a", "X", "Measure", "Measure>b"]], 4), [
            ("x", 1), *WRITE, ("measure", 2, 2),
            ("measure", 3, 3), ("measure", 3, "flag_1"),
        ])

    def test_controls_do_not_control_flag_measurement(self):
        self.assertEqual(build([["•", "◦", "Measure>a"]], 3), [
            ("measure", 2, 2), ("measure", 2, "flag_0"),
        ])

    def test_swap_qft_detector_and_conditional_order(self):
        self.assertEqual(build([["Measure>a"],
                               ["YDetector", "Swap", "Swap", "QFT1", "X<a", "Measure>b"]], 6),
                         WRITE + [("sdg", 0), ("h", 0), ("swap", 1, 2),
                                  ("if", "flag_0", 1), ("x", 4), ("end_if",), ("h", 3),
                                  ("measure", 0, 0), ("measure", 5, 5),
                                  ("measure", 5, "flag_1"), ("h", 0), ("s", 0)])

    def test_conditional_before_overlapping_qft_span(self):
        self.assertEqual(build([["Measure>a"], ["QFT2", "H<a"]], 2), WRITE + [
            ("if", "flag_0", 1), ("h", 1), ("end_if",),
            ("swap", 0, 1), ("h", 0), ("cp", math.pi/2, 1, 0), ("h", 1),
        ])

    def test_display_after_projection_measurement_inverse_detector_before_reset(self):
        qc = FakeCircuit()
        flags = circuit_module.MeasurementFlags()
        basis = [0] * 6
        apply_column_to_qiskit(qc, ["Measure>a"], 6, basis, flag_sources=flags)
        apply_column_to_qiskit(
            qc, ["XDetector", "Measure>b", "|+⟩⟨+|", "X<a", "Bloch", "ZDetectControlReset"],
            6, basis, flag_sources=flags, save_displays=lambda: qc.append(("display",)),
        )
        self.assertEqual(qc.ops, WRITE + [
            ("measure", 5, 5), ("h", 0),
            ("postselect", 2, "|+⟩⟨+|", [5], []),
            ("if", "flag_0", 1), ("mcx", [5], 3), ("end_if",),
            ("detector_projector", 0, [5], []),
            ("measure", 1, 1), ("measure", 1, "flag_1"),
            ("h", 0), ("display",), ("reset", 5),
        ])

    def test_runner_display_builder_preserves_flags(self):
        request = parse_run_request({
            "qubits": 2, "columns": [["Measure>a"], [1, "X<a"], [1, "Probability"]],
            "outputs": {"probability": [{"gate_id": 7, "column": 2, "span": 1, "base_bit": 1}]},
        })
        qc = FakeCircuit()
        add_display_saves(qc, request, lambda axis: axis)
        self.assertEqual(qc.ops, WRITE + [("if", "flag_0", 1), ("x", 1), ("end_if",),
                                        ("probability", [1], "probability:7")])


def gate_case(token, controls):
    def test(self):
        target = len(controls)
        plain = build([controls + [token]], target + 1)
        self.assertEqual(build([["Measure>a"], controls + [token + "<a"]], target + 1),
                         WRITE + [("if", "flag_0", 1)] + plain + [("end_if",)])
    return test


setattr(ConditionalFakeTests, "test_sqrt_x_url_alias", gate_case("X^½", []))


for _token in IFABLE:
    for _label, _controls in (("bare", []), ("positive", ["•"]),
                              ("negative", ["◦"]), ("mixed", ["•", "◦"])):
        setattr(ConditionalFakeTests, f"test_gate_{IFABLE.index(_token)}_{_label}",
                gate_case(_token, _controls))


def invalid_case(token):
    def test(self):
        error = None
        try:
            build([[token]])
        except CircuitBuildError as exc:
            error = exc
        self.assertIsInstance(error, CircuitBuildError)
    return test


# GateFlag::split_token rejects non-ifable suffixes, even blank suffixes.
for _index, _token in enumerate(("P<a", "P(π_2)<a", "Rx<a", "Ry<a", "Rz<a",
                                 "Rx(π_2)<a", "P< ", "Measure<a", "Swap<a", "X>a")):
    setattr(ConditionalFakeTests, f"test_reject_non_ifable_{_index}", invalid_case(_token))


def immutable_case(later):
    def test(self):
        ops = build([["Measure>a"]] + later + [["X<a"]])
        self.assertEqual(ops[-3:], [("if", "flag_0", 1), ("x", 0), ("end_if",)])
    return test


for _label, _later in (("other_name", [["Measure>b"]]), ("plain", [["Measure"]]),
                        ("reset", [["ZDetectControlReset"]])):
    setattr(ConditionalFakeTests, f"test_source_immutable_after_{_label}", immutable_case(_later))


def basis_case(token, initial, expected):
    def test(self):
        flags = circuit_module.MeasurementFlags()
        qc = FakeCircuit()
        basis = [None, initial]
        apply_column_to_qiskit(qc, ["Measure>a"], 2, basis, flag_sources=flags)
        apply_column_to_qiskit(qc, [1, token + "<a"], 2, basis, flag_sources=flags)
        self.assertEqual(basis, [None, expected])
    return test


for _token, _initial, _expected in (("X", 0, None), ("Y", 1, None), ("H", 0, None),
                                    ("√X", 0, None), ("Z", 1, 1), ("S", 0, 0),
                                    ("T†", None, None)):
    setattr(ConditionalFakeTests, f"test_basis_join_{IFABLE.index(_token)}",
            basis_case(_token, _initial, _expected))


REFERENCE_MATRICES = {
    **{key.upper(): value for key, value in MATRICES.items()},
    "S†": MATRICES["sdg"], "T†": ((1, 0), (0, cmath.exp(-1j * math.pi / 4))),
    "Y": ((0, -1j), (1j, 0)),
    "√X": (((1+1j)/2, (1-1j)/2), ((1-1j)/2, (1+1j)/2)),
}


def controlled(state, wire, matrix, controls, antis):
    transformed = single_gate(state, wire, matrix)
    return [transformed[i] if all(i & (1 << c) for c in controls)
            and all(not i & (1 << c) for c in antis) else value
            for i, value in enumerate(state)]


def split_measurement(branch, wire, name=None):
    weight, flags, state = branch
    result = []
    for outcome in (0, 1):
        projected = [a if (i >> wire) & 1 == outcome else 0j for i, a in enumerate(state)]
        probability = sum(abs(a)**2 for a in projected)
        if probability < 1e-14:
            continue
        normalized = [a / math.sqrt(probability) for a in projected]
        updated = dict(flags)
        if name:
            updated[name] = outcome
        result.append((weight * probability, updated, normalized))
    return result


def exact(columns, qubits, *, normalize=True):
    branches = [(1.0, {}, [1+0j] + [0j] * ((1 << qubits)-1))]
    for column in columns:
        controls = [i for i, t in enumerate(column) if t == "•"]
        antis = [i for i, t in enumerate(column) if t == "◦"]
        updated = []
        for weight, flags, state in branches:
            for wire, raw in enumerate(column):
                if not isinstance(raw, str):
                    continue
                token, sep, name = raw.partition("<")
                if sep and name.strip() and not flags.get(name.strip(), 0):
                    continue
                if token in REFERENCE_MATRICES:
                    state = controlled(state, wire, REFERENCE_MATRICES[token], controls, antis)
                elif token in {"|0⟩⟨0|", "|1⟩⟨1|"}:
                    keep = int(token == "|1⟩⟨1|")
                    state = [a if (i >> wire) & 1 == keep else 0j for i, a in enumerate(state)]
                    probability = sum(abs(a)**2 for a in state)
                    weight *= probability
                    state = [a / math.sqrt(probability) for a in state] if probability else state
                elif token not in {"", "•", "◦", "Measure", "Bloch", "Probability", "Density", "Amps1", "Amps2", "Amps3"} and not token.startswith("Measure>"):
                    raise ValueError(f"unsupported reference token {raw!r}")
            current = [(weight, flags, state)] if weight > 1e-14 else []
            for wire, raw in enumerate(column):
                if raw == "Measure" or isinstance(raw, str) and raw.startswith("Measure>"):
                    name = raw.partition(">")[2].strip()
                    current = [new for branch in current for new in split_measurement(branch, wire, name)]
            updated.extend(current)
        branches = updated
    total = sum(p for p, _, _ in branches)
    return [(p/total, flags, state) for p, flags, state in branches] if normalize and total else branches


def density(branches, wire):
    result = [[0j, 0j], [0j, 0j]]
    for weight, _, state in branches:
        for i in range(len(state)):
            if not i & (1 << wire):
                pair = (state[i], state[i | (1 << wire)])
                for row in range(2):
                    for col in range(2):
                        result[row][col] += weight * pair[row] * pair[col].conjugate()
    return result


def probabilities(branches, qubits):
    return [sum(p * abs(state[i])**2 for p, _, state in branches) for i in range(1 << qubits)]


# BB84-style: retain two independent random choices while reusing/resetting q0.
BB84 = [["H"], ["Measure>aliceX"], ["X<aliceX"], ["H"], ["Measure>aliceH"],
        ["X<aliceH"], [1, "X<aliceX"], [1, "H<aliceH"]]
# Teleport an arbitrary complex input T H|0> using two classical corrections.
TELEPORT = [["H", "H"], ["T"], [1, "•", "X"], ["•", "X"], ["H"],
            ["Measure>phase", "Measure>flip"], [1, 1, "X<flip"], [1, 1, "Z<phase"]]
TELEPORT_DECODED = TELEPORT + [[1, 1, "T†"], [1, 1, "H"]]
REUSE = [["H"], ["Measure>a"], ["X<a"], ["Measure>b"], [1, "X<a"]]
OVERWRITE = [["X"], ["Measure>a", "Measure>a"], [1, "X<a"]]
SAME_COLUMN = [["X"], ["Measure>a", "X<a"], [1, "X<a"]]
POSTSELECT = [["H"], ["Measure>a"], [1, "X<a"], ["|1⟩⟨1|"]]
INTERLEAVED = [["H"], ["Measure>a"], ["|1⟩⟨1|"], ["Measure>b"], [1, "X<b"]]


class ConditionalReferenceTests(unittest.TestCase):
    def test_webgpu_applied(self):
        self.assertEqual(probabilities(exact([["X"], ["Measure>a"], [1, "X<a"]], 2), 2),
                         [0, 0, 0, 1])

    def test_webgpu_skipped_and_unset(self):
        self.assertEqual([probabilities(exact(c, 2), 2) for c in
                          ([["Measure>a"], [1, "X<a"]], [[1, "X<b"]])], [[1, 0, 0, 0]]*2)

    def test_bb84_ensemble(self):
        self.assertAlmostEqual(density(exact(BB84, 2), 1)[0][0].real, 0.5)

    def test_teleported_complex_input(self):
        rho = density(exact(TELEPORT, 3), 2)
        expected = [[0.5, cmath.exp(-1j*math.pi/4)/2], [cmath.exp(1j*math.pi/4)/2, 0.5]]
        self.assertLess(max(abs(rho[r][c]-expected[r][c]) for r in range(2) for c in range(2)), 1e-12)


    def test_teleportation_decodes_to_zero(self):
        self.assertLess(abs(density(exact(TELEPORT_DECODED, 3), 2)[0][0]-1), 1e-12)


class ConditionalAerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            load_qiskit()
        except RunnerUnavailable as exc:
            raise unittest.SkipTest(str(exc))

    def run_columns(self, columns, qubits, outputs=None):
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(parse_run_request({
            "qubits": qubits, "columns": columns, "shots": 8192, "seed": 8128,
            "outputs": {"histogram": True, **(outputs or {})},
        }))

    def test_teleportation_decodes_to_deterministic_output(self):
        response = self.run_columns(TELEPORT_DECODED, 3)
        histogram = response["histogram"]
        self.assertEqual((sum(histogram.values()), {key[0] for key in histogram}), (8192, {"0"}))

    def test_prefix_display_not_conditioned_by_later_postselection(self):
        response = self.run_columns(POSTSELECT, 2, {"probability": [
            {"gate_id": 1, "column": 2, "span": 1, "base_bit": 1},
            {"gate_id": 2, "column": 3, "span": 1, "base_bit": 1},
        ]})
        actual = [p for output in response["probability"] for p in output["probabilities"]]
        self.assertLessEqual(max(abs(a-b) for a, b in zip(actual, [0.5, 0.5, 0, 1])), 0.05)

    def test_amplitude_after_deterministic_feed_forward(self):
        columns = [["H"], ["Measure>a"], ["X<a"], ["H"]]
        response = self.run_columns(columns, 1, {"amplitudes": [{
            "gate_id": 1, "column": 3, "span": 1, "base_bit": 0,
            "control_mask": 0, "control_value": 0, "phase_lock_enabled": False,
        }]})
        expected = exact(columns, 1)[0][2]
        actual = [complex(*pair) for pair in response["amplitudes"][0]["ket"]]
        self.assertLess(max(abs(a-b) for a, b in zip(actual, expected)), 1e-9)

    def test_amplitude_is_one_saved_trajectory_not_mean_amplitudes(self):
        # save_amplitudes(pershot=False), unlike the other saves, returns one
        # trajectory. Do not manufacture a pure state from a mixed ensemble.
        columns = [["H"], ["Measure>a"], ["H"]]
        response = self.run_columns(columns, 1, {"amplitudes": [{
            "gate_id": 1, "column": 2, "span": 1, "base_bit": 0,
            "control_mask": 0, "control_value": 0, "phase_lock_enabled": False,
        }]})
        actual = [complex(*pair) for pair in response["amplitudes"][0]["ket"]]
        error = min(max(abs(a-b) for a, b in zip(actual, state))
                    for _, _, state in exact(columns, 1))
        self.assertLess(error, 1e-9)


def histogram_case(columns, qubits):
    def test(self):
        histogram = self.run_columns(columns, qubits)["histogram"]
        expected = probabilities(exact(columns, qubits), qubits)
        # Whole-dict comparison includes stripping flag bits, aggregating shots,
        # retaining quantum-register endianness, and rejecting auxiliary keys.
        total = sum(histogram.values())
        actual = {key: count/max(total, 1) for key, count in histogram.items()}
        wanted = {format(i, f"0{qubits}b"): p for i, p in enumerate(expected) if p > 1e-12}
        survival = sum(p for p, _, _ in exact(columns, qubits, normalize=False))
        error = max([abs(actual.get(k, 0)-wanted.get(k, 0)) for k in actual.keys() | wanted.keys()]
                    + [abs(total/8192-survival)])
        self.assertLessEqual(error, 0.05, (actual, wanted))
    return test


def display_case(columns, qubits, wire, kind):
    def test(self):
        column = len(columns)-1
        output = {"gate_id": 1, "column": column}
        output.update({"wire": wire} if kind == "bloch" else {"span": 1, "base_bit": wire})
        response = self.run_columns(columns, qubits, {kind: [output]})[kind][0]
        rho = density(exact(columns, qubits), wire)
        if kind == "probability":
            actual, expected = response["probabilities"], [rho[0][0].real, rho[1][1].real]
        elif kind == "bloch":
            actual, expected = response["vector"], [2*rho[1][0].real, 2*rho[1][0].imag,
                                                    (rho[0][0]-rho[1][1]).real]
        else:
            actual = [complex(*pair) for pair in response["cells"]] + [response["unity"]]
            expected = [rho[r][c] for r in range(2) for c in range(2)] + [1]
        self.assertLessEqual(max(abs(a-b) for a, b in zip(actual, expected)), 0.05)
    return test


for _label, _columns, _qubits, _wire in (
    ("reuse", REUSE, 2, 1), ("overwrite", OVERWRITE, 2, 1),
    ("same_column", SAME_COLUMN, 2, 1), ("bb84", BB84, 2, 1),
    ("teleport", TELEPORT, 3, 2), ("postselect", POSTSELECT, 2, 1),
    ("applied", [["X"], ["Measure>a"], [1, "X<a"]], 2, 1),
    ("skipped", [["Measure>a"], [1, "X<a"]], 2, 1),
    ("unset", [[1, "X<missing"]], 2, 1),
    ("quantum_control", [["H", "H"], ["Measure>a"], [1, "•", "X<a"]], 3, 2),
    ("quantum_anti", [["H", "H"], ["Measure>a"], [1, "◦", "X<a"]], 3, 2),
    ("interleaved", INTERLEAVED, 2, 1),
):
    setattr(ConditionalAerTests, f"test_histogram_{_label}", histogram_case(_columns, _qubits))
    for _kind in ("probability", "bloch", "densities"):
        setattr(ConditionalAerTests, f"test_{_kind}_{_label}", display_case(_columns, _qubits, _wire, _kind))


for _index, _token in enumerate(IFABLE):
    # Off-axis input distinguishes all nine outputs and the identity; an
    # eigenstate such as |+i> would hide a missing Y or an H/X/Z mix-up.
    _columns = [["X", "H"], [1, "T"], [1, "H"], [1, "T"],
                ["Measure>a"], [1, _token + "<a"]]
    setattr(ConditionalAerTests, f"test_gate_density_{_index}",
            display_case(_columns, 2, 1, "densities"))


if __name__ == "__main__":
    unittest.main()
