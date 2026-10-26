import cmath
import itertools
import math
import unittest

from qni_qiskit_backend.circuit import (
    apply_column_to_qiskit, apply_columns_to_qiskit, column_parity_targets,
)
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import QiskitRunner, RunnerUnavailable, load_qiskit
from test_axis_detectors import MATRICES, single_gate
from test_conditional_gates import FakeCircuit


def build(columns, qubits):
    qc = FakeCircuit()
    apply_columns_to_qiskit(qc, columns, qubits)
    return qc.ops


def column_ops(column, qubits=None):
    return build([column], qubits or len(column))


def shell(parity, body):
    xs = [wire for wire, axis in parity if axis == "xpar"]
    anchor = parity[0][0]
    reduction = [("cx", wire, anchor) for wire, _ in parity[1:]]
    return ([("h", wire) for wire in xs] + reduction + body
            + list(reversed(reduction)) + [("h", wire) for wire in reversed(xs)])


class ParityFakeTests(unittest.TestCase):
    def test_z_three_chain_order(self):
        self.assertEqual(column_ops(["zpar", "zpar", "zpar", "X"]),
                         [("cx", 1, 0), ("cx", 2, 0), ("mcx", [0], 3),
                          ("cx", 2, 0), ("cx", 1, 0)])

    def test_all_x_basis_changes_before_reduction(self):
        self.assertEqual(column_ops(["xpar", "xpar", "xpar", "X"]), shell(
            [(0, "xpar"), (1, "xpar"), (2, "xpar")], [("mcx", [0], 3)]))

    def test_topmost_mixed_anchor_and_ordinary_control_order(self):
        self.assertEqual(column_ops(["xpar", "•", "zpar", "◦", "X"]), shell(
            [(0, "xpar"), (2, "zpar")],
            [("x", 3), ("mcx", [1, 0, 3], 4), ("x", 3)]))

    def test_reverse_mixed_axes_and_target_above_anchor(self):
        self.assertEqual(column_ops(["X", "zpar", "xpar"]), shell(
            [(1, "zpar"), (2, "xpar")], [("mcx", [1], 0)]))

    def test_swap_before_ordinary_targets(self):
        self.assertEqual(column_ops(["zpar", "Swap", "xpar", "Swap", "X"]), shell(
            [(0, "zpar"), (2, "xpar")], [("mcx", [0, 1], 3),
            ("mcx", [0, 3], 1), ("mcx", [0, 1], 3), ("mcx", [0], 4)]))

    def test_equal_angle_phase_group_joins_after_parity_anchor(self):
        self.assertEqual(column_ops(["zpar", "P", "zpar", "P(π_2)"]), shell(
            [(0, "zpar"), (2, "zpar")], [("controlled", ("P", None), [0, 3], [1])]))

    def test_distinct_angle_phase_groups_stay_separate(self):
        self.assertEqual(column_ops(["zpar", "P", "xpar", "P(π)"]), shell(
            [(0, "zpar"), (2, "xpar")], [("controlled", ("P", None), [0], [1]),
            ("controlled", ("P", "π"), [0], [3])]))

    def test_conditional_inside_parity_shell(self):
        self.assertEqual(build([["Measure>a"], ["zpar", "xpar", "X<a"]], 3),
                         [("measure", 0, 0), ("measure", 0, "flag_0")] + shell(
                             [(0, "zpar"), (1, "xpar")],
                             [("if", "flag_0", 1), ("mcx", [0], 2), ("end_if",)]))

    def test_unset_condition_does_not_generate_cz(self):
        self.assertEqual(column_ops(["zpar", "zpar", "•", "X<missing"]), shell(
            [(0, "zpar"), (1, "zpar")], []))

    def test_postselection_before_qft(self):
        self.assertEqual(column_ops(["zpar", "QFT1", "xpar", "|+⟩⟨+|"]), shell(
            [(0, "zpar"), (2, "xpar")], [("postselect", 3, "|+⟩⟨+|", [0], []),
            ("controlled", ("H", None), [0], [1])]))

    def test_detector_measurement_inverse_callback_reset_then_uncompute(self):
        qc = FakeCircuit()
        apply_column_to_qiskit(
            qc, ["xpar", "XDetector", "zpar", "YDetector", "Measure>a",
                 "ZDetectControlReset", "X", "QFT1"], 8, [0] * 8,
            save_displays=lambda: qc.append(("display",)),
        )
        self.assertEqual(qc.ops, [("measure", 5, 5)] + shell([(0, "xpar"), (2, "zpar")], [
            ("h", 1), ("sdg", 3), ("h", 3),
            ("mcx", [5, 0], 6), ("controlled", ("H", None), [5, 0], [7]),
            ("detector_projector", 1, [5, 0], []),
            ("detector_projector", 3, [5, 0], []),
            ("measure", 4, 4), ("measure", 4, "flag_0"),
            ("h", 3), ("s", 3), ("h", 1), ("display",), ("reset", 5),
        ]))

    def test_plain_measure_ignores_parity_but_stays_inside_shell(self):
        self.assertEqual(column_ops(["zpar", "xpar", "Measure"]), shell(
            [(0, "zpar"), (1, "xpar")], [("measure", 2, 2)]))

    def test_named_measure_writes_both_bits_inside_shell(self):
        self.assertEqual(column_ops(["xpar", "zpar", "Measure>a"]), shell(
            [(0, "xpar"), (1, "zpar")], [("measure", 2, 2), ("measure", 2, "flag_0")]))

    def test_controls_only_no_cz_even_with_two_ordinary_controls(self):
        self.assertEqual(column_ops(["•", "zpar", "•", "xpar"]), shell(
            [(1, "zpar"), (3, "xpar")], []))

    def test_detector_limit_counts_only_gathered_parity_anchor(self):
        ops = column_ops(["zpar"] * 9 + ["XDetector"])
        self.assertEqual([op for op in ops if op[0] == "detector_projector"],
                         [("detector_projector", 9, [0], [])])

    def test_detector_aux_includes_parity_anchor_with_ordinary_controls(self):
        self.assertEqual(column_ops(["•"] * 7 + ["zpar", "XDetector"]), [
            ("h", 8), ("detector_projector_aux", 8, list(range(8)), []), ("h", 8),
        ])

    def test_single_parity_wire_needs_no_cx(self):
        self.assertEqual(column_ops(["xpar", "X"]),
                         [("h", 0), ("mcx", [0], 1), ("h", 0)])

    def test_qft_span_drops_covered_parity_before_choosing_anchor(self):
        self.assertEqual(column_ops(["QFT2", "xpar", "zpar", "xpar"]), shell(
            [(2, "zpar"), (3, "xpar")], [
                ("mcx", [2, 0], 1), ("mcx", [2, 1], 0), ("mcx", [2, 0], 1),
                ("controlled", ("H", None), [2], [0]),
                ("controlled", ("P", math.pi / 2), [2, 1], [0]),
                ("controlled", ("H", None), [2], [1]),
            ]))

    def test_qft_all_parity_covered_has_no_shell(self):
        self.assertEqual(column_ops(["QFT3", "xpar", "zpar"]), [
            ("swap", 0, 2), ("h", 0), ("cp", math.pi / 2, 1, 0),
            ("cp", math.pi / 4, 2, 0), ("h", 1), ("cp", math.pi / 2, 2, 1), ("h", 2),
        ])


def parametric_case(token, gate, angle):
    def test(self):
        self.assertEqual(column_ops(["zpar", "xpar", token]), shell(
            [(0, "zpar"), (1, "xpar")], [("controlled", (gate, angle), [0], [2])]))
    return test


for _token, _gate, _angle in (("Rx(π_3)", "RX", "π/3"), ("Ry", "RY", None),
                              ("Rz(-π_2)", "RZ", "-π/2"), ("P(π_4)", "P", "π/4")):
    setattr(ParityFakeTests, f"test_parametric_{_gate}", parametric_case(_token, _gate, _angle))


def span_case(token):
    def test(self):
        self.assertEqual(column_ops([token, "xpar", "zpar", "xpar", "X"]), shell(
            [(2, "zpar"), (3, "xpar")], [("mcx", [2], 4)]))
    return test


for _token in ("Probability2", "Amps2", "Density2"):
    setattr(ParityFakeTests, f"test_span_drop_{_token}", span_case(_token))


def sample_acceptance_case(token, parity):
    def test(self):
        anchor = parity[0][0]
        self.assertEqual(column_ops([token, "xpar", "zpar", "xpar", "X"]),
                         shell(parity, [("mcx", [anchor], 4)]))
    return test


def sample_span_case(token, expected):
    def test(self):
        self.assertEqual(column_parity_targets([token, "xpar", "zpar", "xpar"], 4), expected)
    return test


for _token, _expected in (("Sample1", [(1, "xpar"), (2, "zpar"), (3, "xpar")]),
                          ("Sample2", [(2, "zpar"), (3, "xpar")])):
    setattr(ParityFakeTests, f"test_accept_parity_compilation_{_token}",
            sample_acceptance_case(_token, _expected))
    setattr(ParityFakeTests, f"test_helper_span_drop_{_token}", sample_span_case(_token, _expected))


def display_case(token):
    def test(self):
        qc = FakeCircuit()
        apply_column_to_qiskit(qc, ["xpar", "zpar", token], 3, [0] * 3,
                               save_displays=lambda: qc.append(("display",)))
        self.assertEqual(qc.ops, shell([(0, "xpar"), (1, "zpar")], [("display",)]))
    return test


for _token in ("Bloch", "Probability", "Amps1", "Density"):
    setattr(ParityFakeTests, f"test_display_inside_shell_no_cz_{_token}", display_case(_token))


GATES = {key.upper(): value for key, value in MATRICES.items()}
GATES.update({"S†": MATRICES["sdg"], "T†": ((1, 0), (0, cmath.exp(-1j * math.pi / 4))),
              "Y": ((0, -1j), (1j, 0))})


def active(index, column):
    parity = [wire for wire, token in enumerate(column) if token in {"zpar", "xpar"}]
    return (not parity or sum((index >> wire) & 1 for wire in parity) % 2 == 1) and all(
        ((index >> wire) & 1) == (token == "•")
        for wire, token in enumerate(column) if token in {"•", "◦"})


def apply_predicate(state, wire, matrix, predicate):
    changed = single_gate(state, wire, matrix)
    return [changed[index] if predicate(index) else value for index, value in enumerate(state)]


def project(branch, predicate, flag=None, keep=None):
    weight, flags, state = branch
    result = []
    for outcome in ((0, 1) if keep is None else (keep,)):
        vector = [value if int(predicate(i)) == outcome else 0j for i, value in enumerate(state)]
        mass = sum(abs(value) ** 2 for value in vector)
        if mass > 1e-14:
            result.append((weight * mass, {**flags, **({flag: outcome} if flag else {})},
                           [value / math.sqrt(mass) for value in vector]))
    return result


def frame(state, column):
    """Direct bijection into the display frame; intentionally no CX program."""
    wires = [wire for wire, token in enumerate(column) if token in {"zpar", "xpar"}]
    for wire in wires:
        if column[wire] == "xpar":
            state = single_gate(state, wire, MATRICES["h"])
    result = [0j] * len(state)
    for index, value in enumerate(state):
        odd = sum((index >> wire) & 1 for wire in wires) % 2
        mapped = index ^ ((1 << wires[0]) if wires and odd != ((index >> wires[0]) & 1) else 0)
        result[mapped] = value
    return result


def reference(columns, qubits, *, display_frame=False):
    branches = [(1.0, {}, [1 + 0j] + [0j] * ((1 << qubits) - 1))]
    for column_index, column in enumerate(columns):
        xs = [wire for wire, token in enumerate(column) if token == "xpar"]
        updated = []
        for weight, flags, state in branches:
            for wire in xs:
                state = single_gate(state, wire, MATRICES["h"])
            for wire, raw in enumerate(column):
                if not isinstance(raw, str):
                    continue
                token, _, condition = raw.partition("<")
                if condition and not flags.get(condition, 0):
                    continue
                if token in GATES:
                    state = apply_predicate(state, wire, GATES[token], lambda i: active(i, column))
            current = [(weight, flags, state)]
            for wire, token in enumerate(column):
                if token in {"XDetector", "YDetector"}:
                    rotated = []
                    for p, f, vector in current:
                        if token == "YDetector":
                            vector = single_gate(vector, wire, MATRICES["sdg"])
                        vector = single_gate(vector, wire, MATRICES["h"])
                        rotated.extend(project((p, f, vector),
                                               lambda i: active(i, column) and bool(i & (1 << wire))))
                    current = [(p, f, single_gate(vector, wire, MATRICES["h"])) for p, f, vector in rotated]
                    if token == "YDetector":
                        current = [(p, f, single_gate(vector, wire, MATRICES["s"])) for p, f, vector in current]
                elif token in {"|0⟩⟨0|", "|1⟩⟨1|"}:
                    wanted = int(token == "|1⟩⟨1|")
                    current = [new for branch in current for new in project(
                        branch, lambda i: active(i, column) and ((i >> wire) & 1) != wanted, keep=0)]
                elif token == "Measure" or isinstance(token, str) and token.startswith("Measure>"):
                    name = token.partition(">")[2] or None
                    current = [new for branch in current for new in project(
                        branch, lambda i: bool(i & (1 << wire)), flag=name)]
            for p, f, vector in current:
                for wire in reversed(xs):
                    vector = single_gate(vector, wire, MATRICES["h"])
                if display_frame and column_index == len(columns) - 1:
                    vector = frame(vector, column)
                updated.append((p, f, vector))
        branches = updated
    return branches


def primitive_state(ops, state):
    """Small emitted-unitary interpreter, separate from the column oracle."""
    for op in ops:
        name = op[0]
        if name in MATRICES:
            state = single_gate(state, op[1], MATRICES[name])
        elif name in {"cx", "mcx"}:
            controls, target = ([op[1]], op[2]) if name == "cx" else op[1:]
            state = apply_predicate(state, target, MATRICES["x"],
                                    lambda i: all(i & (1 << c) for c in controls))
        else:
            raise ValueError(f"unsupported primitive {op!r}")
    return state


class ParityReferenceTests(unittest.TestCase):
    def test_two_satisfied_z_controls_do_not_apply(self):
        self.assertEqual(primitive_state(column_ops(["zpar", "zpar", "X"]),
                                        [0j] * 3 + [1 + 0j] + [0j] * 4),
                         [0j] * 3 + [1 + 0j] + [0j] * 4)

    def test_controls_only_is_identity_on_complex_superposition(self):
        state = [cmath.exp(0.37j * i) / math.sqrt(16) for i in range(16)]
        actual = primitive_state(column_ops(["•", "xpar", "•", "zpar"]), state)
        self.assertLess(max(abs(a - b) for a, b in zip(actual, state)), 1e-12)


def exhaustive_case(axes, ordinary, bits):
    def test(self):
        column = list(axes) + list(ordinary) + ["X"]
        qubits = len(column)
        preparation = [["X" if bit else 1 for bit in bits]]
        expected = reference(preparation + [column], qubits)[0][2]
        initial = [0j] * (1 << qubits)
        initial[sum(bit << wire for wire, bit in enumerate(bits))] = 1 + 0j
        actual = primitive_state(column_ops(column), initial)
        self.assertLess(max(abs(a - b) for a, b in zip(actual, expected)), 1e-12)
    return test


for _axes in (("zpar",) * 2, ("xpar",) * 2, ("zpar",) * 3, ("xpar",) * 3,
              ("xpar", "zpar"), ("zpar", "xpar"), ("xpar", "zpar", "xpar")):
    for _ordinary in ((), ("•",), ("◦",), ("•", "◦")):
        for _bits in itertools.product((0, 1), repeat=len(_axes) + len(_ordinary) + 1):
            _name = "_".join(_axes) + "_" + str(len(_ordinary)) + "_" + "".join(_ordinary) + "_" + "".join(map(str, _bits))
            setattr(ParityReferenceTests, "test_basis_" + _name, exhaustive_case(_axes, _ordinary, _bits))


class ParityAerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            load_qiskit()
        except RunnerUnavailable as exc:
            raise unittest.SkipTest(str(exc))

    def run_columns(self, columns, qubits, outputs=None):
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(parse_run_request({
            "qubits": qubits, "columns": columns, "shots": 4096, "seed": 8128,
            "outputs": {"histogram": True, **(outputs or {})},
        }))


AER_CASES = {
    "z_superposition": [["H", "H"], ["T"], ["zpar", "zpar", "X"]],
    "x_superposition": [["H", "H"], ["T", "S"], ["xpar", "xpar", "X"]],
    "mixed_superposition": [["H", "H"], ["T", "S"], ["xpar", "zpar", "X"]],
    "and_positive": [["H", "H", "H"], ["xpar", "zpar", "•", "X"]],
    "and_negative": [["H", "H", "H"], ["zpar", "xpar", "◦", "X"]],
    "x_detector": [["H", "H", "H"], ["T", "T", "T"], ["xpar", "zpar", "XDetector"]],
    "y_detector": [["H", "H", "H"], ["T", "T", "T"], ["zpar", "xpar", "YDetector"]],
    "postselect": [["H", "H", "H"], ["xpar", "zpar", "|1⟩⟨1|"], [1, 1, "H"]],
    "conditional": [["H", "H", 1, "H"], ["T", "S"], [1, 1, 1, "Measure>a"],
                    ["xpar", "zpar", "X<a"]],
    "unset": [["H", "H"], ["zpar", "xpar", "X<missing"]],
    "two_ones": [["X", "X"], ["zpar", "zpar", "X"]],
    "controls_only": [["H", "H", "H", "H"], ["•", "xpar", "•", "zpar"]],
}


def fixture_effect_case(columns):
    def test(self):
        qubits = max(map(len, columns))
        omitted = columns[:-1] + [[1 if token in {"X", "X<a"} else token
                                  for token in columns[-1]]]
        expected = reference(columns, qubits)
        inactive = reference(omitted, qubits)
        differences = [abs(sum(p * abs(state[i]) ** 2 for p, _, state in expected)
                           - sum(p * abs(state[i]) ** 2 for p, _, state in inactive))
                       for i in range(1 << qubits)]
        self.assertGreater(max(differences), 0.05)
    return test


for _label in ("z_superposition", "x_superposition", "mixed_superposition", "conditional"):
    setattr(ParityReferenceTests, f"test_aer_fixture_detects_omitted_target_{_label}",
            fixture_effect_case(AER_CASES[_label]))


def histogram_case(columns):
    def test(self):
        qubits = max(map(len, columns))
        branches = reference(columns, qubits)
        survival = sum(p for p, _, _ in branches)
        expected = [sum(p * abs(state[i]) ** 2 for p, _, state in branches) / survival
                    for i in range(1 << qubits)]
        histogram = self.run_columns(columns, qubits)["histogram"]
        total = sum(histogram.values())
        errors = [abs(histogram.get(format(i, f"0{qubits}b"), 0) / max(total, 1) - p)
                  for i, p in enumerate(expected)] + [abs(total / 4096 - survival)]
        self.assertLessEqual(max(errors), 0.05)
    return test


for _label, _columns in AER_CASES.items():
    setattr(ParityAerTests, "test_histogram_" + _label, histogram_case(_columns))


def conditional_density(branches, target, mask, value):
    rho = [[0j, 0j], [0j, 0j]]
    total = sum(p for p, _, _ in branches)
    for p, _, state in branches:
        for i in range(len(state)):
            if i & (1 << target) or i & mask != value:
                continue
            pair = (state[i], state[i | (1 << target)])
            for row in range(2):
                for col in range(2):
                    rho[row][col] += p / total * pair[row] * pair[col].conjugate()
    return rho


# These saves MUST see the transformed parity frame, not the final state.
# Prepare q0 in S|+>, q1 in H T|+>, and copy q0 into q2 before the
# display column. Both q1 values have nonzero, unequal weight: gathering
# changes which target outcomes the anchor selects, with a unique dominant
# amplitude rest slice. The display occupies its own cell (no overlapping X).


def frame_display_case(kind, axes=("zpar", "zpar")):
    def test(self):
        token = {"probability": "Probability", "densities": "Density",
                 "amplitudes": "Amps1", "bloch": "Bloch"}[kind]
        columns = [["H", "H"], ["S", "T"], [1, "H"], ["•", 1, "X"], list(axes) + [token]]
        request = {"gate_id": 7, "column": 4, "control_mask": 1, "control_value": 1}
        request.update({"wire": 2} if kind == "bloch" else {"span": 1, "base_bit": 2})
        if kind == "amplitudes":
            request["phase_lock_enabled"] = False
        actual = self.run_columns(columns, 3, {kind: [request]})[kind][0]
        branches = reference(columns, 3, display_frame=True)
        rho = conditional_density(branches, 2, 1, 1)
        unity = (rho[0][0] + rho[1][1]).real
        if kind == "probability":
            got = actual["probabilities"]
            wanted = [rho[i][i].real / unity for i in range(2)]
        elif kind == "densities":
            got = [complex(*pair) for pair in actual["cells"]] + [actual["unity"]]
            wanted = [rho[r][c] for r in range(2) for c in range(2)] + [unity]
        elif kind == "bloch":
            got = actual["vector"]
            wanted = [2 * rho[1][0].real / unity, 2 * rho[1][0].imag / unity,
                      (rho[0][0] - rho[1][1]).real / unity]
        else:
            # Unequal slice weights avoid backend-dependent tie breaking.
            state = branches[0][2]
            slices = [[state[rest | 1 | (outcome << 2)] for outcome in range(2)] for rest in (0, 2)]
            best = max(slices, key=lambda vector: sum(abs(a) ** 2 for a in vector))
            norm = math.sqrt(sum(abs(a) ** 2 for a in best))
            got = [complex(*pair) for pair in actual["ket"]]
            wanted = [a / norm for a in best]
        self.assertLessEqual(max(abs(a - b) for a, b in zip(got, wanted)), 0.05)
    return test


for _axes in (("zpar", "zpar"), ("xpar", "zpar")):
    for _kind in ("probability", "densities", "amplitudes", "bloch"):
        setattr(ParityAerTests, f"test_transformed_frame_{'_'.join(_axes)}_{_kind}",
                frame_display_case(_kind, _axes))


def span_state_case(token, bits):
    def test(self):
        column = [token, "xpar", "zpar", "xpar", "X"]
        initial = [0j] * 32
        initial[sum(bit << wire for wire, bit in enumerate(bits))] = 1 + 0j
        expected = reference([["X" if bit else 1 for bit in bits],
                              [1, 1, "zpar", "xpar", "X"]], 5)[0][2]
        actual = primitive_state(column_ops(column), initial)
        self.assertLess(max(abs(a - b) for a, b in zip(actual, expected)), 1e-12)
    return test


for _token in ("Probability2", "Amps2", "Density2"):
    for _bits in itertools.product((0, 1), repeat=5):
        setattr(ParityReferenceTests, f"test_span_predicate_{_token}_{''.join(map(str, _bits))}",
                span_state_case(_token, _bits))


def final_display_case(preparation, kind):
    def test(self):
        qubits = max(map(len, preparation))
        target = 2
        token = "Probability" if kind == "probability" else "Density"
        columns = preparation + [[1, 1, token]]
        output = {"gate_id": 8, "column": len(columns) - 1, "base_bit": target, "span": 1,
                  "control_mask": 0, "control_value": 0}
        actual = self.run_columns(columns, qubits, {kind: [output]})[kind][0]
        rho = conditional_density(reference(columns, qubits), target, 0, 0)
        if kind == "probability":
            got, wanted = actual["probabilities"], [rho[i][i].real for i in range(2)]
        else:
            got = [complex(*pair) for pair in actual["cells"]] + [actual["unity"]]
            wanted = [rho[r][c] for r in range(2) for c in range(2)] + [1]
        self.assertLessEqual(max(abs(a - b) for a, b in zip(got, wanted)), 0.05)
    return test


for _label in ("mixed_superposition", "x_detector", "y_detector", "postselect", "conditional"):
    for _kind in ("probability", "densities"):
        setattr(ParityAerTests, f"test_final_{_label}_{_kind}",
                final_display_case(AER_CASES[_label], _kind))


def full_amplitude_case(axes):
    def test(self):
        columns = [["H", "H", "H"], ["S", "T", "T"], list(axes) + ["X"], ["Amps3"]]
        output = {"gate_id": 9, "column": 3, "span": 3, "base_bit": 0,
                  "control_mask": 0, "control_value": 0, "phase_lock_enabled": False}
        actual = self.run_columns(columns, 3, {"amplitudes": [output]})["amplitudes"][0]
        got = [complex(*pair) for pair in actual["ket"]]
        expected = reference(columns, 3)[0][2]
        self.assertLess(max(abs(a - b) for a, b in zip(got, expected)), 1e-9)
    return test


for _axes in (("zpar", "zpar"), ("xpar", "xpar"), ("xpar", "zpar")):
    setattr(ParityAerTests, f"test_full_register_amplitude_{'_'.join(_axes)}", full_amplitude_case(_axes))


if __name__ == "__main__":
    unittest.main()
