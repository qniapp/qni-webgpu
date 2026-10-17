"""Phase-sensitive controlled gates and sparse controlled-detector regressions."""
import cmath
import math
import unittest

from qni_qiskit_backend.circuit import apply_column_to_qiskit, apply_columns_to_qiskit
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import QiskitRunner, RunnerUnavailable, load_qiskit

from test_axis_detectors import build


R = math.sqrt(.5)


def matrix(gate, theta=math.pi / 2):
    """Independent transcription of apps/web/src/gates/params.rs (including phases)."""
    c, s = math.cos(theta / 2), math.sin(theta / 2)
    return {
        "H": ((R, R), (R, -R)),
        "X": ((0, 1), (1, 0)),
        "Y": ((0, -1j), (1j, 0)),
        "Z": ((1, 0), (0, -1)),
        "√X": ((.5+.5j, .5-.5j), (.5-.5j, .5+.5j)),
        "S": ((1, 0), (0, 1j)),
        "S†": ((1, 0), (0, -1j)),
        "T": ((1, 0), (0, R+1j*R)),
        "T†": ((1, 0), (0, R-1j*R)),
        "P": ((1, 0), (0, cmath.exp(1j*theta))),
        "Rx": ((c, -1j*s), (-1j*s, c)),
        "Ry": ((c, -s), (s, c)),
        "Rz": ((c-1j*s, 0), (0, c+1j*s)),
    }[gate]


def apply(state, target, gate, controls=(), antis=(), theta=math.pi / 2):
    """Sparse little-endian reference; one joint mask/value predicate."""
    m = matrix(gate, theta)
    bit = 1 << target
    mask = sum(1 << wire for wire in (*controls, *antis))
    value = sum(1 << wire for wire in controls)
    result = {}
    for low in {index & ~bit for index in state}:
        a, b = state.get(low, 0j), state.get(low | bit, 0j)
        pair = (m[0][0]*a + m[0][1]*b, m[1][0]*a + m[1][1]*b) if low & mask == value else (a, b)
        for index, amplitude in zip((low, low | bit), pair):
            if abs(amplitude) > 1e-15:
                result[index] = amplitude
    return result


def detector(state, target, axis, controls=(), antis=()):
    """Unconditional axis rotation, {C*|1><1|, I-P}, inverse rotation."""
    rotated = apply(state, target, "S†") if axis == "Y" else state
    rotated = apply(rotated, target, "H")
    mask = sum(1 << wire for wire in (target, *controls, *antis))
    value = sum(1 << wire for wire in (target, *controls))
    branches = []
    for outcome in (0, 1):
        projected = {i: a for i, a in rotated.items() if int(i & mask == value) == outcome}
        probability = sum(abs(a)**2 for a in projected.values())
        if probability > 1e-14:
            normalized = {i: a / math.sqrt(probability) for i, a in projected.items()}
            restored = apply(normalized, target, "H")
            if axis == "Y":
                restored = apply(restored, target, "S")
            branches.append((probability, outcome, restored))
    return branches


def dense(state, qubits):
    return [state.get(i, 0j) for i in range(1 << qubits)]


def rounded(state):
    return {i: (round(complex(a).real, 8), round(complex(a).imag, 8)) for i, a in state.items()}


def distance(actual, expected):
    """Absolute amplitudes, NOT equivalence up to global phase."""
    if len(actual) != len(expected):
        return math.inf
    return max((abs(complex(a)-complex(b)) for a, b in zip(actual, expected)), default=0)


class DetectorAuxOrderTests(unittest.TestCase):
    def test_capacity_counts_logical_qubits_with_detector_and_postselection(self):
        request = parse_run_request({
            "qubits": 32,
            "columns": [["•"]*8 + ["XDetector", "|0⟩⟨0|"]],
        })
        self.assertEqual(request.qubits.value, 32)

    def test_capacity_boundary_builds_aux_projector_without_state_allocation(self):
        self.assertEqual(build([["•"]*8 + ["YDetector"]], 32), [
            ("sdg", 8), ("h", 8),
            ("detector_projector_aux", 8, list(range(8)), []), ("h", 8), ("s", 8),
        ])

    def test_qft_always_follows_ordinary_targets(self):
        self.assertEqual(build([["QFT2", "•", "X"]], 3), [
            ("mcx", [1], 2), ("swap", 0, 1), ("h", 0),
            ("cp", 1.5707963267948966, 1, 0), ("h", 1),
        ])

    def test_mixed_parity_basis_and_gather_surround_aux_projector(self):
        column = ["•"] * 6 + ["◦", "xpar", "zpar", "YDetector"]
        self.assertEqual(build([column], 10), [
            ("h", 7), ("cx", 8, 7), ("sdg", 9), ("h", 9),
            ("detector_projector_aux", 9, list(range(6)) + [7], [6]),
            ("h", 9), ("s", 9), ("cx", 8, 7), ("h", 7),
        ])

    def test_postselection_in_same_column_precedes_aux_projector(self):
        column = ["•"] * 7 + ["◦", "XDetector", "|0⟩⟨0|"]
        self.assertEqual(build([column], 10), [
            ("h", 8), ("postselect", 9, "|0⟩⟨0|", list(range(7)), [7]),
            ("detector_projector_aux", 8, list(range(7)), [7]), ("h", 8),
        ])

    def test_postselection_in_later_column_follows_aux_unrotation(self):
        self.assertEqual(build([["•"] * 8 + ["YDetector"], ["|+⟩⟨+|"]], 9), [
            ("sdg", 8), ("h", 8),
            ("detector_projector_aux", 8, list(range(8)), []),
            ("h", 8), ("s", 8), ("postselect", 0, "|+⟩⟨+|", [], []),
        ])

    def test_two_large_detectors_are_measured_in_wire_order(self):
        self.assertEqual(build([["•"] * 8 + ["XDetector", "YDetector"]], 10), [
            ("h", 8), ("sdg", 9), ("h", 9),
            ("detector_projector_aux", 8, list(range(8)), []),
            ("detector_projector_aux", 9, list(range(8)), []),
            ("h", 9), ("s", 9), ("h", 8),
        ])


class SparseReferenceTests(unittest.TestCase):
    def test_control_mask_value_literal(self):
        self.assertEqual(rounded(apply({0: .5, 1: .5j, 5: -.5, 7: -.5j}, 3, "Y", (0, 2), (1,))),
                         {0: (.5, 0), 1: (0, .5), 13: (0, -.5), 7: (0, -.5)})

    def test_x_detector_literal_joint_no_click_coherence(self):
        self.assertEqual([(round(p, 8), o, rounded(s)) for p, o, s in detector({0: R, 1: R}, 1, "X", (0,))],
                         [(.75, 0, {0: (.81649658, 0), 1: (.40824829, 0), 3: (.40824829, 0)}),
                          (.25, 1, {1: (.70710678, 0), 3: (-.70710678, 0)})])

    def test_y_detector_literal_anti_control(self):
        self.assertEqual([(round(p, 8), o, rounded(s)) for p, o, s in detector({0: R, 1: R}, 1, "Y", (), (0,))],
                         [(.75, 0, {0: (.40824829, 0), 2: (0, .40824829), 1: (.81649658, 0)}),
                          (.25, 1, {0: (.70710678, 0), 2: (0, -.70710678)})])


def literal_matrix_test(gate, expected):
    def test(self):
        self.assertEqual(rounded(apply({1: 1}, 0, gate)), expected)
    return test


# Literal columns, not expectations calculated with Qiskit or the reference matrix.
for _gate, _expected in {
    "H": {0: (.70710678, 0), 1: (-.70710678, 0)},
    "X": {0: (1, 0)}, "Y": {0: (0, -1)}, "Z": {1: (-1, 0)},
    "√X": {0: (.5, -.5), 1: (.5, .5)},
    "S": {1: (0, 1)}, "S†": {1: (0, -1)},
    "T": {1: (.70710678, .70710678)}, "T†": {1: (.70710678, -.70710678)},
    "P": {1: (0, 1)},
    "Rx": {0: (0, -.70710678), 1: (.70710678, 0)},
    "Ry": {0: (-.70710678, 0), 1: (.70710678, 0)},
    "Rz": {1: (.70710678, .70710678)},
}.items():
    setattr(SparseReferenceTests, "test_literal_" + _gate, literal_matrix_test(_gate, _expected))


def preparation(qubits):
    """Only two controls superposed: click stays O(1), including at 11 wires."""
    return [["H", "H"] + ["X"]*(qubits-4) + [1, 1],
            ["T", "S"]]


def prepared(qubits):
    state = {0: 1}
    for wire, gate in enumerate(["H", "H"] + ["X"]*(qubits-4) + [None, None]):
        if gate:
            state = apply(state, wire, gate)
    for wire, gate in ((0, "T"), (1, "S")):
        state = apply(state, wire, gate)
    return state


def control_column(qubits, axis):
    return ["•"]*(qubits-2) + ["◦", axis+"Detector"]


def expected_branches(qubits, axis):
    branches = detector(prepared(qubits), qubits-1, axis, tuple(range(qubits-2)), (qubits-2,))
    return [(p, o, apply(s, 0, "H")) for p, o, s in branches]


def probabilities(branches, qubits):
    values = [0.]*(1 << qubits)
    for p, _, state in branches:
        for index, amplitude in state.items():
            values[index] += p*abs(amplitude)**2
    return values


class ControlledAerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            cls.QuantumCircuit, transpile, _, cls.AerSimulator = load_qiskit()
            cls.transpile = staticmethod(transpile)
        except RunnerUnavailable as exc:
            raise unittest.SkipTest(str(exc))

    def simulate(self, qc, shots=512):
        simulator = self.AerSimulator(method="statevector", max_parallel_threads=1)
        return simulator.run(self.transpile(qc, simulator), shots=shots, seed_simulator=8128).result()

    def test_qft_span_control_ordinary_gate_first_complex_amplitudes(self):
        from qiskit.quantum_info import Statevector
        state = [complex(i+1, (-1)**i*(i+2)) for i in range(8)]
        norm = math.sqrt(sum(abs(a)**2 for a in state))
        state = [a/norm for a in state]
        actual, explicit = self.QuantumCircuit(3), self.QuantumCircuit(3)
        apply_columns_to_qiskit(actual, [["QFT2", "•", "X"]], 3)
        # X controlled on wire 1 first; QFT's span excludes that internal control.
        explicit.cx(1, 2)
        apply_columns_to_qiskit(explicit, [["QFT2"]], 3)
        self.assertLess(distance(Statevector(state).evolve(actual).data,
                                 Statevector(state).evolve(explicit).data), 1e-10)

    def test_eight_wire_boundary_uses_kraus_without_auxiliary(self):
        qc = self.QuantumCircuit(8, 8)
        apply_columns_to_qiskit(qc, preparation(8) + [control_column(8, "X")], 8)
        self.assertEqual(([len(op.qubits) for op in qc.data if op.operation.name == "kraus"],
                          qc.num_qubits, qc.num_clbits), ([8], 8, 8))

    def test_nine_wire_boundary_uses_mcx_reset_without_kraus(self):
        qc = self.QuantumCircuit(9, 9)
        apply_columns_to_qiskit(qc, [["•"]*8 + ["YDetector"]], 9)
        names = {op.operation.name for op in qc.data}
        self.assertEqual((qc.num_qubits, qc.num_clbits,
                          [r.name for r in qc.qregs if r.name == "postselection_aux"],
                          names & {"mcx", "reset", "kraus"}),
                         (10, 9, ["postselection_aux"], {"mcx", "reset"}))

    def branch_samples(self, qubits, axis):
        qc = self.QuantumCircuit(qubits, qubits)
        apply_columns_to_qiskit(qc, preparation(qubits) + [control_column(qubits, axis), ["H"]], qubits)
        qc.save_statevector(label="branches", pershot=True)
        result = self.simulate(qc)
        branches = expected_branches(qubits, axis)
        wanted = [dense(s, qc.num_qubits) for _, _, s in branches]
        counts = [0]*len(branches)
        worst = 0.
        states = result.data(0)["branches"]
        for saved in states:
            errors = [distance(saved.data, s) for s in wanted]
            closest = min(range(len(errors)), key=errors.__getitem__)
            counts[closest] += 1
            worst = max(worst, errors[closest])
        return qc, states, worst, counts, branches

    def runner(self, columns, qubits, outputs=None):
        request = parse_run_request({"qubits": qubits, "columns": columns, "shots": 1024,
                                     "seed": 8128, "outputs": {"histogram": True, **(outputs or {})}})
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(request)


def controlled_gate_test(gate, theta, angle_token):
    def test(self):
        from qiskit.quantum_info import Statevector
        raw = {i: complex(i+1, (-1)**i*(17-i)) for i in range(16)}
        norm = math.sqrt(sum(abs(a)**2 for a in raw.values()))
        state = {i: a/norm for i, a in raw.items()}
        qc = self.QuantumCircuit(4)
        token = f"{gate}({angle_token})" if gate in {"P", "Rx", "Ry", "Rz"} else gate
        apply_column_to_qiskit(qc, ["•", "◦", "•", token], 4, [None]*4)
        actual = Statevector(dense(state, 4)).evolve(qc).data
        self.assertLess(distance(actual, dense(apply(state, 3, gate, (0, 2), (1,), theta), 4)), 1e-10)
    return test


for _gate in ("H", "X", "Y", "Z", "√X", "S", "S†", "T", "T†", "P", "Rx", "Ry", "Rz"):
    for _label, _theta, _token in (("decimal", .7, "0.7"), ("pi_third", math.pi/3, "pi/3")):
        setattr(ControlledAerTests, f"test_controlled_{_gate}_{_label}", controlled_gate_test(_gate, _theta, _token))


def detector_case(qubits, axis, check):
    def test(self):
        qc, states, worst, counts, branches = self.branch_samples(qubits, axis)
        if check == "states":
            error, tolerance = worst, 1e-9
        elif check == "frequency":
            error = max(abs(count/len(states)-p) for count, (p, _, _) in zip(counts, branches))
            tolerance = .05
        else:
            actual = ([r.name for r in qc.qregs if r.name == "postselection_aux"],
                      qc.num_qubits, qc.num_clbits, len(states))
            error, tolerance = int(actual != (["postselection_aux"], qubits+1, qubits, 512)), 0
        self.assertLessEqual(error, tolerance)
    return test


for _qubits in (8, 10, 11):
    for _axis in ("X", "Y"):
        for _check in (("states", "frequency") if _qubits == 8 else ("states", "frequency", "aux")):
            setattr(ControlledAerTests, f"test_detector_{_qubits}_{_axis}_{_check}", detector_case(_qubits, _axis, _check))


def runner_detector_case(qubits, axis, output):
    def test(self):
        columns = preparation(qubits) + [control_column(qubits, axis), ["H"], ["Probability"+str(qubits)]]
        outputs = {"probability": [{"gate_id": 1, "column": 4, "span": qubits, "base_bit": 0}]} if output == "probability" else {}
        response = self.runner(columns, qubits, outputs)
        wanted = probabilities(expected_branches(qubits, axis), qubits)
        if output == "probability":
            actual = response["probability"][0]["probabilities"]
        else:
            actual = [response["histogram"].get(format(i, f"0{qubits}b"), 0)/1024 for i in range(1 << qubits)]
        self.assertLess(distance(actual, wanted), .05)
    return test


for _qubits in (10, 11):
    for _axis in ("X", "Y"):
        for _output in ("histogram", "probability"):
            setattr(ControlledAerTests, f"test_runner_{_qubits}_{_axis}_{_output}", runner_detector_case(_qubits, _axis, _output))


def sharing_columns(axis, order, same):
    """Nine predicate wires plus a disjoint postselection target."""
    detector_column = ["•"]*8 + [axis+"Detector", 1]
    projection = [1]*9 + ["|0⟩⟨0|"]
    if same:
        if order == "detector_first":
            return [["H", "H"] + ["X"]*6 + [1, "H"], detector_column[:9] + ["|0⟩⟨0|"]]
        # Reverse target wire order without changing predicate size.
        return [["H", "H"] + ["X"]*7 + ["H"], [1]*9 + ["T"],
                ["|0⟩⟨0|"] + ["•"]*8 + [axis+"Detector"]]
    return [["H", "H"] + ["X"]*6 + [1, "H"]] + ([detector_column, projection] if order == "detector_first" else [projection, detector_column])


def sharing_case(axis, order, same, check):
    def test(self):
        qc = self.QuantumCircuit(10, 10)
        apply_columns_to_qiskit(qc, sharing_columns(axis, order, same), 10)
        if check == "registers":
            actual = ([r.name for r in qc.qregs if r.name == "postselection_aux"],
                      qc.num_qubits, qc.num_clbits)
            error = int(actual != (["postselection_aux"], 11, 11))
        else:
            qc.save_statevector(label="shared", pershot=True)
            states = self.simulate(qc).data(0)["shared"]
            error = max(abs(complex(a)) for saved in states for a in saved.data[1 << 10:])
        self.assertLessEqual(error, 1e-10)
    return test


for _axis in ("X", "Y"):
    for _order in ("detector_first", "postselection_first"):
        for _same in (False, True):
            for _check in ("registers", "reset"):
                setattr(ControlledAerTests, f"test_shared_{_axis}_{_order}_{'same' if _same else 'later'}_{_check}",
                        sharing_case(_axis, _order, _same, _check))


def leakage_response(self, axis):
    # Control wire 1 stays zero: detector is inactive but must allocate its aux.
    # Postselect wire 0 after a measurement, so filtering is nontrivial.
    n = 10
    columns = [["H"], ["Measure"], ["Sample1"],
               [1] + ["•"]*8 + [axis+"Detector"],
               ["|1⟩⟨1|"], ["Sample1"]]
    outputs = {
        "amplitudes": [{"gate_id": i, "column": col, "span": 1, "base_bit": 0,
                        "control_mask": 0, "control_value": 0,
                        "phase_lock_enabled": False} for i, col in ((1, 2), (2, 5))],
        "probability": [{"gate_id": i, "column": col, "span": 1, "base_bit": 0}
                        for i, col in ((3, 2), (4, 5))],
        "bloch": [{"gate_id": i, "column": col, "wire": 0} for i, col in ((5, 2), (6, 5))],
        "densities": [{"gate_id": i, "column": col, "span": 1, "base_bit": 0}
                      for i, col in ((7, 2), (8, 5))],
    }
    return self.runner(columns, n, outputs)


def leakage_case(axis, check):
    def test(self):
        response = leakage_response(self, axis)
        tolerance = .05
        if check == "histogram_keys":
            error, tolerance = int(set(response["histogram"]) != {"0000000001"}), 0
        elif check == "survival":
            error = abs(sum(response["histogram"].values())/1024-.5)
        elif check == "probability_sample":
            error = distance([p for d in response["probability"] for p in d["probabilities"]], [.5, .5, 0, 1])
        elif check == "amplitude_shape":
            error, tolerance = int([len(d["ket"]) for d in response["amplitudes"]] != [2, 2]), 0
        elif check == "early_amplitudes":
            ket = [complex(*pair) for pair in response["amplitudes"][0]["ket"]]
            error, tolerance = min(distance(ket, [1, 0]), distance(ket, [0, 1])), 1e-9
        elif check == "amplitudes":
            # save_amplitudes returns one shot, not an ensemble average.
            # Every kept shot at the later save is exactly |1>.
            ket = [complex(*pair) for pair in response["amplitudes"][1]["ket"]]
            error, tolerance = distance(ket, [0, 1]), 1e-9
        elif check == "bloch":
            error = distance([v for d in response["bloch"] for v in d["vector"]], [0, 0, 0, 0, 0, -1])
            tolerance = .1  # Bloch Z is twice the probability sampling error.
        else:
            cells = [complex(*pair) for d in response["densities"] for pair in d["cells"]]
            error = distance(cells, [.5, 0, 0, .5, 0, 0, 0, 1])
        self.assertLessEqual(error, tolerance)
    return test


for _axis in ("X", "Y"):
    for _check in ("histogram_keys", "survival", "probability_sample", "amplitude_shape", "early_amplitudes", "amplitudes", "bloch", "density"):
        setattr(ControlledAerTests, f"test_aux_leakage_{_axis}_{_check}", leakage_case(_axis, _check))


if __name__ == "__main__":
    unittest.main()
