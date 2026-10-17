"""Sample uses probability saves; CPU oracles below are strictly test-only.

Exact branch references interpret columns, not emitted primitives. The seeded
sampler ports probability_render.wgsl with explicit u32 wrap and f32 rounding;
it is not a backend sampling implementation or a GPU execution test.
"""
import math
import struct
import unittest

from qni_qiskit_backend.circuit import (
    CircuitBuildError, apply_column_to_qiskit, apply_columns_to_qiskit,
    column_parity_targets,
)
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import (
    QiskitRunner, RunnerUnavailable, add_display_saves, load_qiskit,
    probability_display_response, probability_save_qargs,
)
from test_conditional_gates import FakeCircuit
from test_parity_controls import primitive_state, reference, shell


def output(column, span=1, base_bit=0, mask=0, value=0, gate_id=7):
    return dict(gate_id=gate_id, column=column, span=span, base_bit=base_bit,
                control_mask=mask, control_value=value)


def request(columns, qubits, outputs):
    return parse_run_request(dict(qubits=qubits, columns=columns, shots=2048,
                                  seed=8128, outputs={"probability": outputs}))


def saved_ops(columns, qubits, outputs):
    qc = FakeCircuit()
    add_display_saves(qc, request(columns, qubits, outputs), lambda axis: axis)
    return qc.ops


def sample_reference(columns, qubits):
    # Span coverage is interpreted directly, not via column_parity_targets.
    # Other covered operations retain their existing compiler semantics.
    effective = []
    for column in columns:
        covered = set()
        for wire, token in enumerate(column):
            if token in {"Sample1", "Sample2", "Sample3"}:
                covered.update(range(wire, min(wire + int(token[-1]), qubits)))
        effective.append([1 if wire in covered and token in {"xpar", "zpar"} else token
                          for wire, token in enumerate(column)])
    return reference(effective, qubits, display_frame=True)


def marginal(branches, span, base_bit, mask=0, value=0):
    """Direct conditional marginal over full indices, including overlap controls."""
    masses = [0.0] * (1 << span)
    for weight, _, state in branches:
        for index, amplitude in enumerate(state):
            if index & mask == value:
                masses[(index >> base_bit) & ((1 << span) - 1)] += weight * abs(amplitude) ** 2
    total = sum(masses)
    return [mass / total for mass in masses] if total > 1e-12 else masses


def saved_marginal(branches, spec, qubits):
    """Emulate only Aer save qarg packing, then use the production response path."""
    qargs = probability_save_qargs(spec, qubits)
    packed = [0.0] * (1 << len(qargs))
    for weight, _, state in branches:
        for index, amplitude in enumerate(state):
            row = sum(((index >> wire) & 1) << bit for bit, wire in enumerate(qargs))
            packed[row] += weight * abs(amplitude) ** 2
    return probability_display_response(spec, packed, qubits)["probabilities"]


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def sample_unit(seed):
    s = ((seed & 0xffffffff) * 0x9E3779B9 + 0x85EBCA6B) & 0xffffffff
    s ^= s >> 13
    s = (s * 0xC2B2AE35) & 0xffffffff
    s ^= s >> 16
    return f32(f32(s >> 8) / 16777216.0)


INVALID = 0xffffffff


def sampled_outcome(probabilities, span, seed):
    """Literal probability_render.wgsl sampler (probability_prob clamps input)."""
    if span == 0 or span > 3:
        return INVALID
    probabilities = [f32(min(1.0, max(0.0, p))) for p in probabilities[:1 << span]]
    total = 0.0
    for probability in probabilities:
        total = f32(total + probability)
    if total <= f32(1e-12):
        return INVALID
    threshold = f32(sample_unit(seed) * total)
    cdf, last = 0.0, INVALID
    for index, probability in enumerate(probabilities):
        if probability > 0:
            last = index
        cdf = f32(cdf + probability)
        if threshold < cdf:
            return index
    return last


class SampleFakeTests(unittest.TestCase):
    def test_save_is_existing_probability_protocol(self):
        self.assertEqual(saved_ops([["Sample3"]], 3, [output(0, 3)]),
                         [("probability", [0, 1, 2], "probability:7")])

    def test_sample_span_clamped_at_register_bottom(self):
        self.assertEqual(saved_ops([[1, 1, "Sample3"]], 3, [output(0, 1, 2)]),
                         [("probability", [2], "probability:7")])

    def test_controls_and_sample_do_not_synthesize_control_only_cz(self):
        self.assertEqual(saved_ops([["•", "◦", "Sample1"]], 3,
                                   [output(0, base_bit=2, mask=3, value=1)]),
                         [("probability", [2, 0, 1], "probability:7")])

    def test_save_after_projection_measure_inverse_detector_before_reset(self):
        self.assertEqual(saved_ops(
            [["XDetector", "YDetector", "Measure>a", "|+⟩⟨+|", "Sample1", "ZDetectControlReset"]],
            6, [output(0, base_bit=4)]), [
                ("measure", 5, 5), ("h", 0), ("sdg", 1), ("h", 1),
                ("postselect", 3, "|+⟩⟨+|", [5], []),
                ("detector_projector", 0, [5], []), ("detector_projector", 1, [5], []),
                ("measure", 2, 2), ("measure", 2, "flag_0"),
                ("h", 1), ("s", 1), ("h", 0),
                ("probability", [4], "probability:7"), ("reset", 5),
            ])

    def test_save_inside_mixed_parity_shell(self):
        self.assertEqual(saved_ops([["xpar", "zpar", "Sample1"]], 3,
                                   [output(0, base_bit=2, mask=1, value=1)]),
                         shell([(0, "xpar"), (1, "zpar")],
                               [("probability", [2, 0], "probability:7")]))

    def test_sample_span_drops_covered_parity_before_anchor(self):
        self.assertEqual(saved_ops([["Sample2", "xpar", "zpar", "xpar", "X"]], 5,
                                   [output(0, 2, mask=4, value=4)]),
                         shell([(2, "zpar"), (3, "xpar")], [
                             ("mcx", [2], 4), ("probability", [0, 1, 2], "probability:7")]))

    def test_save_after_conditional_with_persistent_flag(self):
        self.assertEqual(saved_ops([["Measure>a"], ["Sample1", "X<a"]], 2,
                                   [output(1)]), [
                             ("measure", 0, 0), ("measure", 0, "flag_0"),
                             ("if", "flag_0", 1), ("x", 1), ("end_if",),
                             ("probability", [0], "probability:7")])

    def test_save_column_order_not_request_order(self):
        self.assertEqual(saved_ops([["Sample1"], ["X"], ["Sample1"]], 1,
                                   [output(2, gate_id=9), output(0)]), [
                             ("probability", [0], "probability:7"), ("x", 0),
                             ("probability", [0], "probability:9")])

    def test_sample_preserves_basis_tracker(self):
        basis = [0, 1, None]
        apply_column_to_qiskit(FakeCircuit(), ["Sample3"], 3, basis)
        self.assertEqual(basis, [0, 1, None])

    def test_controls_sample_column_is_identity_on_complex_state(self):
        qc = FakeCircuit()
        apply_columns_to_qiskit(qc, [["•", "xpar", "zpar", "Sample1"]], 4)
        state = [complex(math.cos(i), math.sin(i)) / 4 for i in range(16)]
        self.assertLess(max(abs(a-b) for a, b in zip(primitive_state(qc.ops, state), state)), 1e-12)


def span_case(span):
    def test(self):
        parsed = request([[f"Sample{span}"]], span, [output(0, span)])
        self.assertEqual((parsed.columns, parsed.probability_outputs[0].span),
                         ([[f"Sample{span}"]], span))
    return test


def drop_case(span):
    def test(self):
        self.assertEqual(column_parity_targets([f"Sample{span}", "xpar", "zpar", "xpar"], 4),
                         [(i, t) for i, t in ((1, "xpar"), (2, "zpar"), (3, "xpar")) if i >= span])
    return test


for _span in (1, 2, 3):
    setattr(SampleFakeTests, f"test_parse_span_{_span}", span_case(_span))
    setattr(SampleFakeTests, f"test_parity_drop_span_{_span}", drop_case(_span))


def rejected_case(token):
    def test(self):
        with self.assertRaises(CircuitBuildError):
            apply_columns_to_qiskit(FakeCircuit(), [[token]], 4)
    return test


for _token in ("Sample", "Sample0", "Sample4", "Sample16", "Sample1<a"):
    setattr(SampleFakeTests, "test_reject_" + _token.replace("<", "_"), rejected_case(_token))


# These fixtures are deliberately small; exact branch arithmetic needs no numpy.
CASES = {
    "positive_control": ([["H", "H"], ["•", "Sample1"]], 2, 1, 1, 1, 1),
    "negative_control": ([["H", "H"], ["◦", "Sample1"]], 2, 1, 1, 1, 0),
    "zero_condition": ([[1, "X"], ["•", "Sample1"]], 2, 1, 1, 1, 1),
    "overlap_control": ([["H", "H", "H"], [1, "Sample2"]], 3, 2, 1, 6, 4),
    "postselect": ([["H", "H"], ["•", "X"], ["|1⟩⟨1|", "Sample1"]], 2, 1, 1, 0, 0),
    "conditional": ([["H"], ["Measure>a"], [1, "X<a"], [1, "Sample1"]], 2, 1, 1, 0, 0),
    "unset": ([["X<missing"], ["Sample1"]], 1, 1, 0, 0, 0),
    "measure": ([["H", "H"], ["Measure", "Sample1"]], 2, 1, 1, 0, 0),
    "x_detector": ([["H", "H"], ["T", "S"], ["XDetector", "Sample1"]], 2, 1, 1, 0, 0),
    "y_detector": ([["H", "H"], ["T", "T"], ["YDetector", "Sample1"]], 2, 1, 1, 0, 0),
    "controlled_detector": ([["H", "H", "H"], [1, "T"], ["•", "YDetector", "Sample1"]], 3, 1, 2, 1, 1),
    "z_parity": ([["H", "H"], [1, "T"], [1, "H"], ["•", 1, "X"],
                   ["zpar", "zpar", "Sample1"]], 3, 1, 2, 1, 1),
    "mixed_parity": ([["H", "H"], ["S", "T"], [1, "H"], ["•", 1, "X"],
                       ["xpar", "zpar", "Sample1"]], 3, 1, 2, 1, 1),
    "span2_order": ([["X", 1, "X"], [1, "Sample2"]], 3, 2, 1, 0, 0),
    "span3_order": ([["X", 1, "X"], ["Sample3"]], 3, 3, 0, 0, 0),
    "span_parity_drop": ([["H", "H", "H", "H"], ["S", "T", "T", "S"],
                           ["Sample2", "xpar", "zpar", "xpar", "X"]], 5, 2, 0, 4, 4),
    "postselect_then_sample": ([["H"], ["•", "X"], ["|1⟩⟨1|"],
                                ["Sample2"]], 2, 2, 0, 0, 0),
    "detector_then_sample": ([["H"], ["T"], ["•", "X"], ["YDetector"],
                              ["Sample2"]], 2, 2, 0, 0, 0),
    "measure_then_sample": ([["H"], ["•", "X"], ["Measure"],
                             ["Sample2"]], 2, 2, 0, 0, 0),
}


class SampleReferenceTests(unittest.TestCase):
    def test_little_endian_wire_order(self):
        self.assertEqual(marginal(reference([["X", 1, "X"]], 3), 3, 0),
                         [0, 0, 0, 0, 0, 1, 0, 0])

    def test_postselection_exact_marginal(self):
        self.assertEqual(marginal(sample_reference(CASES["postselect_then_sample"][0], 2), 2, 0),
                         [0, 0, 0, 1])

    def test_conditional_exact_marginal(self):
        self.assertLess(max(abs(p - 0.5) for p in marginal(
            sample_reference(CASES["conditional"][0], 2), 1, 1)), 1e-12)

    def test_overlap_control_exact_marginal(self):
        self.assertEqual(marginal(sample_reference(CASES["overlap_control"][0], 3), 2, 1, 6, 4),
                         [0, 0, 1, 0])

    def test_seeded_sequence_with_zero_mass_bin(self):
        self.assertEqual([sampled_outcome([0.125, 0.25, 0, 0.625], 2, s) for s in range(12)],
                         [1, 1, 3, 3, 3, 3, 1, 0, 3, 3, 1, 0])

    def test_zero_condition_is_unsampleable(self):
        self.assertEqual(sampled_outcome(marginal(reference([[1, "X"]], 2), 1, 1, 1, 1), 1, 7),
                         INVALID)

    def test_seed_u32_wrap(self):
        self.assertEqual(sample_unit(7 + (1 << 32)), sample_unit(7))

    def test_seed_known_hash_values(self):
        self.assertEqual([sample_unit(s) for s in (0, 1, 7, 0xffffffff)],
                         [0.317598819732666, 0.23845124244689941,
                          0.026828646659851074, 0.8804715275764465])

    def test_invalid_spans(self):
        self.assertEqual([sampled_outcome([1] * 16, s, 0) for s in (0, 4)], [INVALID, INVALID])

    def test_negative_undefined_sentinel_clamped(self):
        self.assertEqual(sampled_outcome([-1, -1], 1, 0), INVALID)

    def test_sampler_does_not_mutate_probabilities(self):
        values = [0.25, 0.75]
        sampled_outcome(values, 1, 12)
        self.assertEqual(values, [0.25, 0.75])

    def test_sampler_frequency_matches_mass(self):
        values = [0.125, 0.25, 0, 0.625]
        counts = [0] * 4
        for seed in range(4096):
            counts[sampled_outcome(values, 2, seed)] += 1
        self.assertLess(max(abs(count / 4096 - p) for count, p in zip(counts, values)), 0.03)

    def test_sampler_strict_cdf_boundary(self):
        threshold = sample_unit(0)
        self.assertEqual(sampled_outcome([threshold, 1-threshold], 1, 0), 1)


def reference_case(case):
    def test(self):
        columns, qubits, span, base, mask, value = case
        branches = sample_reference(columns, qubits)
        spec = request(columns, qubits, [output(len(columns)-1, span, base, mask, value)]).probability_outputs[0]
        self.assertLess(max(abs(a-b) for a, b in zip(saved_marginal(branches, spec, qubits),
                                                   marginal(branches, span, base, mask, value))), 1e-12)
    return test


def seeded_case(span):
    def test(self):
        values = [0.0] * (1 << span)
        values[-1] = 0.37
        self.assertEqual({sampled_outcome(values, span, seed) for seed in (0, 1, 17, 0xffffffff)},
                         {(1 << span)-1})
    return test


for _label, _case in CASES.items():
    setattr(SampleReferenceTests, "test_exact_marginal_" + _label, reference_case(_case))
for _span in (1, 2, 3):
    setattr(SampleReferenceTests, f"test_seeded_sparse_span_{_span}", seeded_case(_span))


class SampleAerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            load_qiskit()
        except RunnerUnavailable as exc:
            raise unittest.SkipTest(str(exc))

    def run_sample(self, columns, qubits, outputs):
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(request(columns, qubits, outputs))

    def test_basis_spans_and_bit_order_are_exact(self):
        columns = [["X", 1, "X"], ["Sample1"], ["Sample2"], ["Sample3"], [1, 1, "Sample3"]]
        result = self.run_sample(columns, 3, [
            output(1), output(2, 2, gate_id=8), output(3, 3, gate_id=9),
            output(4, 1, 2, gate_id=10),
        ])["probability"]
        self.assertEqual([item["probabilities"] for item in result],
                         [[0, 1], [0, 1, 0, 0], [0, 0, 0, 0, 0, 1, 0, 0], [0, 1]])

    def test_prefix_save_not_conditioned_on_later_postselection(self):
        columns = [["H"], ["Sample1"], ["|1⟩⟨1|"], ["Sample1"]]
        result = self.run_sample(columns, 1, [output(1), output(3, gate_id=9)])["probability"]
        self.assertLess(max(abs(a-b) for a, b in zip(
            [p for item in result for p in item["probabilities"]], [0.5, 0.5, 0, 1])), 0.06)

    def test_no_state_change_from_sample(self):
        columns = [["H", "H", "H"], ["T", "S"], ["Sample3"], ["H", "H", "H"], ["Sample3"]]
        without = columns[:2] + [[]] + columns[3:]
        actual = self.run_sample(columns, 3, [output(4, 3)])["probability"]
        expected = self.run_sample(without, 3, [output(4, 3)])["probability"]
        self.assertEqual(actual, expected)

    def test_response_preserves_request_order(self):
        result = self.run_sample([["Sample1"], ["X"], ["Sample1"]], 1,
                                 [output(2, gate_id=9), output(0)])["probability"]
        self.assertEqual([(r["gate_id"], r["probabilities"]) for r in result], [(9, [0, 1]), (7, [1, 0])])


def aer_case(case):
    def test(self):
        columns, qubits, span, base, mask, value = case
        wanted = marginal(sample_reference(columns, qubits), span, base, mask, value)
        actual = self.run_sample(columns, qubits, [output(len(columns)-1, span, base, mask, value)])
        got = actual["probability"][0]["probabilities"]
        self.assertLess(max(abs(a-b) for a, b in zip(got, wanted)), 0.06)
    return test


for _label, _case in CASES.items():
    setattr(SampleAerTests, "test_probability_" + _label, aer_case(_case))


if __name__ == "__main__":
    unittest.main()
