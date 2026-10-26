import math
import unittest

from qni_qiskit_backend.circuit import apply_column_to_qiskit
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import QiskitRunner, RunnerUnavailable, load_qiskit
from test_axis_detectors import FakeCircuit, MATRICES, build, single_gate

TOKENS = ("|0⟩⟨0|", "|1⟩⟨1|", "|+⟩⟨+|", "|-⟩⟨-|")
PROJECTORS = (
    ((1, 0), (0, 0)), ((0, 0), (0, 1)),
    ((0.5, 0.5), (0.5, 0.5)), ((0.5, -0.5), (-0.5, 0.5)),
)


def project(state, target, token, controls=(), antis=()):
    matrix = PROJECTORS[TOKENS.index(token)]
    result = single_gate(state, target, matrix)
    result = [result[i] if all(i & (1 << c) for c in controls)
              and all(not i & (1 << c) for c in antis) else amp
              for i, amp in enumerate(state)]
    norm = sum(abs(amp) ** 2 for amp in result)
    return ([amp / math.sqrt(norm) for amp in result], norm) if norm > 1e-12 else ([0j] * len(state), 0.0)


def write(state, target, value, controls=(), antis=()):
    result = state[:]
    for i in range(len(state)):
        if i & (1 << target) or not all(i & (1 << c) for c in controls) or any(i & (1 << c) for c in antis):
            continue
        j = i | (1 << target)
        opposite, desired = (i, j) if value else (j, i)
        if abs(state[opposite]) ** 2 > abs(state[desired]) ** 2 + 1e-6:
            result[i], result[j] = state[j], state[i]
    return result


def measure(state, target, basis="Measure", controls=(), antis=()):
    if basis == "Measure":
        controls, antis = (), ()
    rotated = state[:]
    if basis == "YDetector":
        rotated = single_gate(rotated, target, MATRICES["sdg"])
    if basis in ("XDetector", "YDetector"):
        rotated = single_gate(rotated, target, MATRICES["h"])
    click = [bool(i & (1 << target)) and all(i & (1 << c) for c in controls)
             and all(not i & (1 << c) for c in antis) for i in range(len(state))]
    masses = [sum(abs(a) ** 2 for a, hit in zip(rotated, click) if hit == bool(outcome))
              for outcome in (0, 1)]
    if sum(masses) <= 1e-12:
        return [0j] * len(state), -1
    if min(masses) > 1e-28 * sum(masses):
        raise ValueError("ambiguous measurement fixture")
    outcome = int(masses[1] > masses[0])
    result = [a / math.sqrt(masses[outcome]) if hit == bool(outcome) else 0j
              for a, hit in zip(rotated, click)]
    if basis in ("XDetector", "YDetector"):
        result = single_gate(result, target, MATRICES["h"])
    if basis == "YDetector":
        result = single_gate(result, target, MATRICES["s"])
    return result, outcome


def display(state, kind):
    norm = sum(abs(a) ** 2 for a in state)
    denominator = max(norm, 1e-12)
    if kind == "Probability":
        return [abs(a) ** 2 / denominator if norm > 1e-12 else -1 for a in state]
    if kind == "Amplitude":
        return [(a.real / math.sqrt(denominator), a.imag / math.sqrt(denominator),
                 abs(a) / math.sqrt(denominator)) for a in state]
    if kind == "Density":
        return ([(a * b.conjugate()) / denominator for a in state for b in state], denominator)
    if kind == "Bloch":
        coherence = state[0].conjugate() * state[1]
        return [2 * coherence.real / denominator, 2 * coherence.imag / denominator,
                (abs(state[0]) ** 2 - abs(state[1]) ** 2) / denominator]
    raise ValueError(kind)


def exact(columns, qubits=1):
    state = [1 + 0j] + [0j] * ((1 << qubits) - 1)
    survival = 1.0
    snapshots = []
    rates = []
    outcomes = {}
    for column in columns:
        controls = [i for i, t in enumerate(column) if t == "•"]
        antis = [i for i, t in enumerate(column) if t == "◦"]
        for wire, token in enumerate(column):
            if isinstance(token, str) and "<" in token:
                token, name = token.split("<", 1)
                if outcomes.get(name, -1) != 1:
                    continue
            if isinstance(token, str) and token.startswith("Measure>"):
                state, outcomes[token.split(">", 1)[1]] = measure(state, wire)
            elif token in ("Measure", "XDetector", "YDetector"):
                state, _ = measure(state, wire, token, controls, antis)
            elif token in ("|0>", "|1>"):
                state = write(state, wire, int(token[1]), controls, antis)
            elif isinstance(token, str) and token in ("Ry(π/32)", "Ry(π/16)"):
                angle = math.pi / (32 if token == "Ry(π/32)" else 16)
                c, s = math.cos(angle / 2), math.sin(angle / 2)
                updated = single_gate(state, wire, ((c, -s), (s, c)))
                state = [updated[i] if all(i & (1 << bit) for bit in controls)
                         and all(not i & (1 << bit) for bit in antis) else amp
                         for i, amp in enumerate(state)]
            elif token in TOKENS:
                state, probability = project(state, wire, token, controls, antis)
                survival *= probability
                rates.append(survival)
            elif isinstance(token, str) and token.lower() in MATRICES:
                updated = single_gate(state, wire, MATRICES[token.lower()])
                state = [updated[i] if all(i & (1 << c) for c in controls)
                         and all(not i & (1 << c) for c in antis) else amp
                         for i, amp in enumerate(state)]
        snapshots.append(state[:])
    return state, rates, snapshots


def rounded(values):
    return [round(float(complex(v).real), 5) for v in values]


class PostselectionFakeTests(unittest.TestCase):
    def test_qft_follows_projection_even_on_earlier_wire(self):
        self.assertEqual(build([["QFT1", TOKENS[0]]], 2), [
            ("postselect", 1, TOKENS[0], [], []), ("h", 0),
        ])

    def test_multiple_projectors_before_measurements_and_display(self):
        circuit = FakeCircuit()
        apply_column_to_qiskit(circuit, [TOKENS[2], TOKENS[1], "Measure"], 3, [0]*3,
                              save_displays=lambda: circuit.append(("display",)))
        self.assertEqual(circuit.ops, [
            ("postselect", 0, TOKENS[2], [], []),
            ("postselect", 1, TOKENS[1], [], []), ("measure", 2, 2), ("display",),
        ])

    def test_inactive_controlled_projection_preserves_basis_for_later_write(self):
        self.assertEqual(build([[1, "X"], [TOKENS[0], "◦"], ["|1>"]], 2), [
            ("x", 1), ("postselect", 0, TOKENS[0], [], [1]), ("x", 0),
        ])

    def test_out_of_range_projector(self):
        from qni_qiskit_backend.circuit import CircuitBuildError
        with self.assertRaises(CircuitBuildError):
            build([[1, TOKENS[0]]], 1)


def fake_case(token, control):
    def test(self):
        circuit = FakeCircuit()
        column = ["YDetector", "Swap", "Swap", token, "Measure", "Probability"]
        controls, antis = [], []
        if control:
            column.append(control)
            (controls if control == "•" else antis).append(6)
        apply_column_to_qiskit(circuit, column, len(column), [None] * len(column),
                              save_displays=lambda: circuit.append(("display",)))
        ops = circuit.ops
        if not control:
            expected = [("sdg", 0), ("h", 0), ("swap", 1, 2),
                        ("postselect", 3, token, [], []), ("measure", 0, 0),
                        ("measure", 4, 4), ("h", 0), ("s", 0), ("display",)]
        else:
            flip = [("x", 6)] if antis else []
            swaps = []
            for source, target in ((1, 2), (2, 1), (1, 2)):
                swaps += flip + [("mcx", [source, 6] if antis else [6, source], target)] + flip
            expected = [("sdg", 0), ("h", 0)] + swaps + [
                ("postselect", 3, token, controls, antis),
                ("detector_projector", 0, controls, antis), ("measure", 4, 4),
                ("h", 0), ("s", 0), ("display",)]
        self.assertEqual(ops, expected)
    return test


for _i, _token in enumerate(TOKENS):
    for _j, _control in enumerate((None, "•", "◦")):
        setattr(PostselectionFakeTests, f"test_order_kind_{_i}_control_{_j}", fake_case(_token, _control))


class WebGPUParityTests(unittest.TestCase):
    def test_four_conditional_states(self):
        actual = [rounded(exact([[before], [token]])[0]) for before, token in zip(("H", "H", "H", "X"), TOKENS)]
        self.assertEqual(actual, [[1, 0], [0, 1], [0.70711, 0.70711], [-0.70711, 0.70711]])

    def test_four_acceptance_probabilities(self):
        self.assertEqual([rounded(exact([[before], [token]])[1]) for before, token in zip(("H", "H", "H", "X"), TOKENS)], [[0.5], [0.5], [1], [0.5]])

    def test_bell_partner(self):
        self.assertEqual(rounded(exact([["H"], ["•", "X"], [TOKENS[1]]], 2)[0]), [0, 0, 0, 1])

    def test_successive_survival(self):
        self.assertEqual(rounded(exact([["H"], [TOKENS[0]], ["H"], [TOKENS[1]]])[1]), [0.5, 0.25])

    def test_unchanged_second_survival(self):
        self.assertEqual(rounded(exact([["H"], [TOKENS[0]], ["H"], [TOKENS[2]]])[1]), [0.5, 0.5])

    def test_same_column_survival(self):
        self.assertEqual(rounded(exact([["H", "H"], [TOKENS[0], TOKENS[1]]], 2)[1]), [0.5, 0.25])

    def test_controlled_inactive_slice(self):
        self.assertEqual(rounded(exact([["H", "H"], ["•", TOKENS[0]]], 2)[0]), [0.57735, 0.57735, 0.57735, 0])

    def test_anti_controlled_inactive_slice(self):
        self.assertEqual(rounded(exact([["H", "H"], ["◦", TOKENS[0]]], 2)[0]), [0.57735, 0.57735, 0, 0.57735])

    def test_controlled_global_acceptance(self):
        self.assertEqual(rounded(exact([["H", "H"], ["•", TOKENS[0]]], 2)[1]), [0.75])

    def test_impossible_state_and_survival(self):
        self.assertEqual(exact([["X"], [TOKENS[0]]])[:2], ([0j, 0j], [0.0]))

    def test_orthogonal_x_is_impossible(self):
        self.assertEqual(exact([["H"], [TOKENS[3]]])[0], [0j, 0j])

    def test_epsilon_is_empty(self):
        self.assertEqual(project([1, 1e-7], 0, TOKENS[1]), ([0j, 0j], 0))

    def test_earlier_snapshot_unconditioned(self):
        self.assertEqual(rounded(exact([["H"], [TOKENS[0]]])[2][0]), [0.70711, 0.70711])

    def test_large_reduction_state(self):
        self.assertEqual(rounded(exact([["H"] * 7, [1] * 6 + [TOKENS[0]]], 7)[0]), [0.125] * 64 + [0] * 64)

    def test_large_reduction_survival(self):
        self.assertEqual(rounded(exact([["H"] * 7, [1] * 6 + [TOKENS[0]]], 7)[1]), [0.5])

    def test_plus_h_projector_h_equivalence(self):
        direct = exact([["H"], ["S"], [TOKENS[2]]])[0]
        decomposed = exact([["H"], ["S"], ["H"], [TOKENS[0]], ["H"]])[0]
        self.assertLess(max(abs(a-b) for a, b in zip(direct, decomposed)), 1e-12)

    def test_minus_h_projector_h_equivalence(self):
        direct = exact([["H"], ["S"], [TOKENS[3]]])[0]
        decomposed = exact([["H"], ["S"], ["H"], [TOKENS[1]], ["H"]])[0]
        self.assertLess(max(abs(a-b) for a, b in zip(direct, decomposed)), 1e-12)

    def test_write_basis_values(self):
        self.assertEqual([exact([[before], [token]])[0] for before, token in
                          (("X", "|0>"), (1, "|1>"), ("H", "|0>"), ("H", "|1>"))],
                         [[1, 0], [0, 1], [math.sqrt(0.5)] * 2, [math.sqrt(0.5)] * 2])

    def test_write_is_pairwise_not_global(self):
        self.assertEqual([write([0, 1j, 2, 0], 0, value) for value in (0, 1)],
                         [[1j, 0, 2, 0], [0, 1j, 0, 2]])

    def test_write_compares_squared_magnitudes_with_epsilon(self):
        # Amplitude differences exceed epsilon but squared differences do not.
        self.assertEqual([write([0, 0.0009j], 0, 0), write([0.0009j, 0], 0, 1),
                          write([0, 0.0011j], 0, 0), write([0.0011j, 0], 0, 1)],
                         [[0, 0.0009j], [0.0009j, 0], [0.0011j, 0], [0, 0.0011j]])

    def test_write_control_and_anti_control(self):
        self.assertEqual([write([0, 1, 0, 2], 0, 0, controls=(1,)),
                          write([0, 1, 0, 2], 0, 0, antis=(1,))],
                         [[0, 1, 2, 0], [1, 0, 0, 2]])

    def test_write_epsilon_boundary_does_not_swap(self):
        self.assertEqual(write([0, 0.001], 0, 0), [0, 0.001])

    def test_inactive_detector_keeps_coherent_target(self):
        state = exact([["H"], ["S"]], 2)[0]
        actual, outcome = measure(state, 0, "YDetector", controls=(1,))
        self.assertEqual((rounded(actual), rounded([a.imag for a in actual]), outcome),
                         (rounded(state), rounded([a.imag for a in state]), 0))

    def test_inactive_anti_control_measurement_has_no_click(self):
        self.assertEqual(measure([0, 0, 0, 1], 0, "ZDetector", antis=(1,)), ([0, 0, 0, 1], 0))

    def test_detector_no_click_preserves_inactive_control_coherence(self):
        state = exact([[1, "H"]], 2)[0]
        actual, outcome = measure(state, 0, "ZDetector", controls=(1,))
        self.assertEqual((rounded(actual), outcome), (rounded(state), 0))

    def test_exact_axis_detectors_preserve_definite_eigenstates(self):
        self.assertEqual([rounded(exact(columns)[0]) for columns in
                          ([["H"], [TOKENS[2]], ["XDetector"]],
                           [["H"], [TOKENS[2]], ["S"], ["YDetector"]])],
                         [[0.70711, 0.70711], [0.70711, 0]])

    def test_detectors_and_unitaries_cannot_revive_zero(self):
        self.assertEqual(exact([[TOKENS[1]], ["XDetector"], ["YDetector"],
                                ["H"], ["S"], ["X"]])[0], [0j, 0j])

    def test_measure_ignores_unsatisfied_column_controls(self):
        self.assertEqual(measure([0, 1, 0, 0], 0, controls=(1,)), ([0, 1, 0, 0], 1))

    def test_measure_normalized_conditional_state(self):
        self.assertEqual(exact([["H"], [TOKENS[1]], ["Measure"]])[0], [0, 1])

    def test_measure_definite_outcomes_and_normalization(self):
        self.assertEqual([measure([2j, 0], 0), measure([0, 2j], 0)],
                         [([1j, 0], 0), ([0, 1j], 1)])

    def test_measure_rejects_ambiguous_branches(self):
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            measure([math.sqrt(0.5)] * 2, 0)

    def test_detector_rejects_ambiguous_branches(self):
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            measure([1, 0], 0, "XDetector")

    def test_measure_cutoff_is_inclusive(self):
        self.assertEqual(measure([1e-6, 0], 0), ([0j, 0j], -1))

    def test_measure_above_cutoff_is_defined(self):
        self.assertEqual(measure([1.001e-6, 0], 0), ([1, 0], 0))

    def test_project_cutoff_is_inclusive(self):
        self.assertEqual(project([0, 1e-6], 0, TOKENS[1]), ([0j, 0j], 0))

    def test_project_above_cutoff_normalizes(self):
        self.assertEqual(project([0, 2e-6j], 0, TOKENS[1]), ([0j, 1j], 4e-12))

    def test_later_operations_cannot_revive_state_or_survival(self):
        self.assertEqual(exact([["X"], [TOKENS[0]], ["|1>"], ["Measure"],
                                ["|0>"], [TOKENS[2]]])[:2], ([0j, 0j], [0, 0]))

    def test_undefined_measurement_leaves_classical_control_inactive(self):
        self.assertEqual(exact([["X"], [TOKENS[0]], ["Measure>a"], ["X<a"]])[0], [0j, 0j])

    def test_defined_measurement_activates_classical_control(self):
        self.assertEqual(exact([["X"], ["Measure>a"], ["X<a"]])[0], [1, 0])

    def test_probability_conditional_display(self):
        self.assertEqual(display(exact([["H"], [TOKENS[0]]])[0], "Probability"), [1, 0])

    def test_amplitude_conditional_display(self):
        self.assertEqual(display(exact([["H"], [TOKENS[0]]])[0], "Amplitude")[0], (1, 0, 1))

    def test_bloch_conditional_display(self):
        self.assertEqual(display(exact([["H"], [TOKENS[0]]])[0], "Bloch"), [0, 0, 1])

    def test_density_conditional_display(self):
        self.assertEqual(display(exact([["H"], [TOKENS[0]]])[0], "Density"), ([1, 0, 0, 0], 1))

    def test_probability_impossible_display_is_all_undefined(self):
        self.assertEqual(display(exact([[TOKENS[1]]], 2)[0], "Probability"), [-1] * 4)

    def test_density_impossible_display_has_epsilon_unity(self):
        self.assertEqual(display(exact([["X"], [TOKENS[0]]])[0], "Density"), ([0j] * 4, 1e-12))

    def test_displays_normalize_nonunit_state(self):
        self.assertEqual([display([2j, 0j], kind) for kind in
                          ("Probability", "Amplitude", "Bloch", "Density")],
                         [[1, 0], [(0, 1, 1), (0, 0, 0)], [0, 0, 1], ([1, 0, 0, 0], 4)])

    def test_survival_label_fixture_small_kept(self):
        self.assertAlmostEqual(exact([["Ry(π/32)"], [TOKENS[1]]])[1][0], 0.002407636663901557, places=14)

    def test_survival_label_fixture_tiny_loss(self):
        self.assertAlmostEqual(exact([["Ry(π/32)"], [TOKENS[0]]])[1][0], 0.9975923633360985, places=14)

    def test_survival_label_fixture_small_loss(self):
        self.assertAlmostEqual(exact([["Ry(π/16)"], [TOKENS[0]]])[1][0], 0.9903926402016152, places=14)

    def test_survival_label_fixture_below_f32_subtraction_precision(self):
        self.assertAlmostEqual(exact([["Ry(π/16)"] * 4, [TOKENS[1]] * 4], 4)[1][-1],
                               8.519541481627294e-9, delta=1e-20)


def measurement_case(basis, control, outcome):
    def test(self):
        if outcome == -1:
            state = exact([[TOKENS[1]]], 2)[0]
        else:
            prepare = [["H"], [TOKENS[0 if outcome == 0 else 1]]]
            if basis != "Measure":
                prepare.append(["H"])
            if basis == "YDetector":
                prepare.append(["S"])
            state = exact(prepare, 2)[0]
            if control == "•":
                state = single_gate(state, 1, MATRICES["x"])
        actual, measured = measure(state, 0, basis,
                                   (1,) if control == "•" else (),
                                   (1,) if control == "◦" else ())
        self.assertEqual((rounded(actual), rounded([a.imag for a in actual]), measured),
                         (rounded(state), rounded([a.imag for a in state]), outcome))
    return test


for _basis in ("Measure", "XDetector", "YDetector"):
    for _control_index, _control in enumerate((None, "•", "◦")):
        for _outcome in (-1, 0, 1):
            setattr(WebGPUParityTests, f"test_measurement_{_basis}_control_{_control_index}_outcome_{_outcome}",
                    measurement_case(_basis, _control, _outcome))


def run_request(columns, qubits=1, outputs=None, shots=2048):
    return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(parse_run_request({
        "qubits": qubits, "columns": columns, "shots": shots, "seed": 8128,
        "outputs": {"histogram": True, **(outputs or {})},
    }))


def outputs_at(column, span=1):
    return {
        "amplitudes": [{"gate_id": 10, "column": column, "span": span, "base_bit": 0,
                        "control_mask": 0, "control_value": 0, "phase_lock_enabled": False}],
        "bloch": [{"gate_id": 11, "column": column, "wire": 0}],
        "probability": [{"gate_id": 12, "column": column, "span": span, "base_bit": 0}],
        "densities": [{"gate_id": 13, "column": column, "span": span, "base_bit": 0}],
    }


class AerPostselectionParityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            load_qiskit()
        except RunnerUnavailable as exc:
            raise unittest.SkipTest(str(exc))

    def test_probability_exact(self):
        response = run_request([["H"], [TOKENS[0]], ["Probability"]], outputs={"probability": outputs_at(2)["probability"]})
        self.assertEqual(response["probability"][0]["probabilities"], [1, 0])

    def test_amplitude_minus_phase_exact(self):
        response = run_request([["X"], [TOKENS[3]], ["Amps1"]], outputs={"amplitudes": outputs_at(2)["amplitudes"]})
        expected = exact([["X"], [TOKENS[3]]])[0]
        actual = [complex(*pair) for pair in response["amplitudes"][0]["ket"]]
        self.assertLess(max(abs(a-b) for a, b in zip(actual, expected)), 1e-9)

    def test_bloch_exact(self):
        response = run_request([["H"], [TOKENS[0]], ["Bloch"]], outputs={"bloch": outputs_at(2)["bloch"]})
        self.assertEqual(response["bloch"][0]["vector"], [0, 0, 1])

    def test_density_exact(self):
        response = run_request([["H"], [TOKENS[1]], ["Density"]], outputs={"densities": outputs_at(2)["densities"]})
        self.assertEqual(response["densities"][0]["cells"], [[0, 0], [0, 0], [0, 0], [1, 0]])

    def test_earlier_display_is_unconditioned(self):
        response = run_request([["H"], ["Probability"], [TOKENS[1]]], outputs={"probability": outputs_at(1)["probability"]})
        self.assertLess(max(abs(p-0.5) for p in response["probability"][0]["probabilities"]), 1e-9)

    def test_earlier_display_survives_later_impossible_projection(self):
        response = run_request([["X"], ["Probability"], [TOKENS[0]]], outputs={"probability": outputs_at(1)["probability"]})
        self.assertEqual(response["probability"][0]["probabilities"], [0, 1])

    def test_controlled_histogram(self):
        response = run_request([["H", "H"], ["•", TOKENS[0]]], 2, shots=8192)
        total = sum(response["histogram"].values())
        self.assertLess(max(abs(response["histogram"].get(key, 0)/total - value) for key, value in {"00": 1/3, "01": 1/3, "10": 1/3, "11": 0}.items()), 0.05)

    def test_controlled_amplitude_preserves_coherence(self):
        response = run_request([["H", "H"], ["•", TOKENS[0]], ["Amps2"]], 2, {"amplitudes": outputs_at(2, 2)["amplitudes"]})
        actual = [complex(*pair) for pair in response["amplitudes"][0]["ket"]]
        expected = exact([["H", "H"], ["•", TOKENS[0]]], 2)[0]
        self.assertLess(max(abs(a-b) for a, b in zip(actual, expected)), 1e-9)

    def test_inactive_projection_then_write(self):
        self.assertEqual(run_request([[1, "X"], [TOKENS[0], "◦"], ["|1>"]], 2)["histogram"], {"11": 2048})

    def test_zero_controlled_bloch_is_marked_undefined(self):
        outputs = {"bloch": [{"gate_id": 1, "column": 2, "wire": 1,
                              "control_mask": 1, "control_value": 1}]}
        response = run_request([["X"], [TOKENS[0]], ["•", "Bloch"]], 2, outputs)
        self.assertEqual(response["bloch"][0], {"gate_id": 1, "vector": [0, 0, 0], "defined": False})

    def test_zero_histogram(self):
        self.assertEqual(run_request([["X"], [TOKENS[0]]])["histogram"], {})

    def test_zero_probability_undefined(self):
        response = run_request([["X"], [TOKENS[0]], ["Probability"]], outputs={"probability": outputs_at(2)["probability"]})
        self.assertEqual(response["probability"][0]["probabilities"], [-1, -1])

    def test_zero_amplitude_empty(self):
        response = run_request([["X"], [TOKENS[0]], ["Amps1"]], outputs={"amplitudes": outputs_at(2)["amplitudes"]})
        self.assertEqual(response["amplitudes"][0]["ket"], [[0, 0], [0, 0]])

    def test_zero_density_empty(self):
        response = run_request([["X"], [TOKENS[0]], ["Density"]], outputs={"densities": outputs_at(2)["densities"]})
        self.assertEqual(response["densities"][0]["cells"], [[0, 0]] * 4)

    def test_zero_cannot_revive(self):
        self.assertEqual(run_request([["X"], [TOKENS[0]], ["|1>"], ["Measure"], [TOKENS[2]]])["histogram"], {})

    def test_zero_bloch_is_marked_undefined(self):
        response = run_request([["X"], [TOKENS[0]], ["Bloch"]], outputs={"bloch": outputs_at(2)["bloch"]})
        self.assertEqual(response["bloch"][0], {"gate_id": 11, "vector": [0, 0, 0], "defined": False})

    def test_zero_density_unity_matches_webgpu(self):
        response = run_request([["X"], [TOKENS[0]], ["Density"]], outputs={"densities": outputs_at(2)["densities"]})
        self.assertEqual(response["densities"][0]["unity"], 1e-12)

    def test_zero_amplitude_is_marked_undefined(self):
        response = run_request([["X"], [TOKENS[0]], ["Amps1"]], outputs={"amplitudes": outputs_at(2)["amplitudes"]})
        self.assertFalse(response["amplitudes"][0]["defined"])

    def test_later_postselection_does_not_reweight_earlier_display(self):
        outputs = {"probability": [
            {"gate_id": 1, "column": 1, "span": 1, "base_bit": 0},
            {"gate_id": 2, "column": 4, "span": 1, "base_bit": 0},
        ]}
        response = run_request([["H"], ["Probability"], [TOKENS[0]], ["H"], ["Probability"], [TOKENS[1]]], outputs=outputs)
        self.assertLess(max(abs(p-0.5) for out in response["probability"] for p in out["probabilities"]), 1e-9)

    def test_prior_measurement_filters_only_later_display_ensemble(self):
        outputs = {"probability": [
            {"gate_id": 1, "column": 2, "span": 1, "base_bit": 0},
            {"gate_id": 2, "column": 4, "span": 1, "base_bit": 0},
        ]}
        response = run_request([["H"], ["Measure"], ["Probability"], [TOKENS[1]], ["Probability"]], outputs=outputs, shots=8192)
        self.assertLess(max(abs(response["probability"][i]["probabilities"][j] - expected) for i, j, expected in ((0, 0, .5), (0, 1, .5), (1, 0, 0), (1, 1, 1))), .05)

    def test_bell_histogram_is_conditioned(self):
        response = run_request([["H"], ["•", "X"], [TOKENS[1]]], 2)
        self.assertEqual(set(response["histogram"]), {"11"})

    def test_same_column_display_sees_projection(self):
        outputs = {"probability": [{"gate_id": 1, "column": 2, "base_bit": 1, "span": 1}]}
        response = run_request([["H"], ["•", "X"], [TOKENS[1], "Probability"]], 2, outputs)
        self.assertEqual(response["probability"][0]["probabilities"], [0, 1])

    def test_successive_projectors_histogram(self):
        response = run_request([["H"], [TOKENS[0]], ["H"], [TOKENS[1]]])
        self.assertLess(abs(sum(response["histogram"].values())/response["shots"] - .25), .05)

    def test_no_kept_shots_for_possible_rare_event_is_undefined(self):
        outputs = {"probability": outputs_at(2)["probability"]}
        response = run_request([["Ry(0.00001)"], [TOKENS[1]], ["Probability"]], outputs=outputs, shots=1)
        self.assertEqual((response["histogram"], response["probability"][0]["probabilities"]), ({}, [-1, -1]))


def aer_state_case(token, control):
    def test(self):
        column = [token] if control is None else [control, token]
        qubits = len(column)
        columns = [["H"] * qubits, ["S"] * qubits, column, ["Amps" + str(qubits)]]
        response = run_request(columns, qubits, {"amplitudes": outputs_at(3, qubits)["amplitudes"]})
        expected = exact(columns, qubits)[0]
        actual = [complex(*pair) for pair in response["amplitudes"][0]["ket"]]
        self.assertLess(max(abs(a-b) for a, b in zip(actual, expected)), 1e-9)
    return test


for _i, _token in enumerate(TOKENS):
    for _j, _control in enumerate((None, "•", "◦")):
        setattr(AerPostselectionParityTests, f"test_exact_phase_kind_{_i}_control_{_j}", aer_state_case(_token, _control))
