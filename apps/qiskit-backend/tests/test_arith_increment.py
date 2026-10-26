import math
import unittest

from qni_qiskit_backend.circuit import (
    CircuitBuildError, apply_column_to_qiskit, apply_columns_to_qiskit,
    parse_arithmetic_token,
)
from qni_qiskit_backend.contract import parse_run_request
from qni_qiskit_backend.runners import QiskitRunner, RunnerUnavailable, load_qiskit
from test_conditional_gates import FakeCircuit


def build(column, qubits=None):
    qc = FakeCircuit()
    apply_columns_to_qiskit(qc, [column], len(column) if qubits is None else qubits)
    return qc.ops


class ArithmeticFakeTests(unittest.TestCase):
    def test_increment_carry_high_to_low(self):
        self.assertEqual(build(["inc4", 1, 1, 1]), [
            ("mcx", [0, 1, 2], 3), ("mcx", [0, 1], 2), ("mcx", [0], 1), ("x", 0)])

    def test_decrement_reverses_carry(self):
        self.assertEqual(build(["dec4", 1, 1, 1]), [
            ("x", 0), ("mcx", [0], 1), ("mcx", [0, 1], 2), ("mcx", [0, 1, 2], 3)])

    def test_external_controls_and_anti_wrap_each_primitive(self):
        self.assertEqual(build(["•", "inc2", 1, "◦", "•"]), [
            ("x", 3), ("mcx", [0, 4, 1, 3], 2), ("x", 3),
            ("x", 3), ("mcx", [0, 4, 3], 1), ("x", 3)])

    def test_multiple_anti_controls_wrap_in_reverse_order(self):
        self.assertEqual(build(["◦", "dec1", "◦"]), [
            ("x", 0), ("x", 2), ("mcx", [0, 2], 1), ("x", 2), ("x", 0)])

    def test_internal_controls_and_parity_are_ignored(self):
        self.assertEqual(build(["inc4", "•", "◦", "xpar"]), [
            ("mcx", [0, 1, 2], 3), ("mcx", [0, 1], 2), ("mcx", [0], 1), ("x", 0)])

    def test_external_parity_shell_and_covered_marker_dropped(self):
        self.assertEqual(build(["inc2", "xpar", "zpar", "xpar"]), [
            ("h", 3), ("cx", 3, 2), ("mcx", [2, 0], 1), ("mcx", [2], 0),
            ("cx", 3, 2), ("h", 3)])

    def test_ordinary_targets_before_shared_resizable_row_order(self):
        # JSON contains rows, not insertion IDs; stable row order mirrors QFT.
        self.assertEqual(build(["inc2", "•", "QFT1", "dec1", "X", "Measure"]), [
            ("mcx", [1], 4), ("mcx", [0], 1), ("x", 0),
            ("controlled", ("H", None), [1], [2]), ("mcx", [1], 3), ("measure", 5, 5)])

    def test_qft_above_arithmetic_keeps_row_order(self):
        self.assertEqual(build(["QFT1", "inc1", "dec1", "X"]), [
            ("x", 3), ("h", 0), ("x", 1), ("x", 2)])

    def test_known_active_span_is_conservatively_unknown(self):
        basis = [0, 0, 0]
        apply_column_to_qiskit(FakeCircuit(), ["inc2", "◦", "X"], 3, basis)
        self.assertEqual(basis, [None, None, 1])

    def test_known_inactive_external_control_preserves_span(self):
        basis = [0, 0, 0]
        apply_column_to_qiskit(FakeCircuit(), ["•", "inc2"], 3, basis)
        self.assertEqual(basis, [0, 0, 0])

    def test_known_inactive_external_anti_preserves_span(self):
        basis = [1, 0, 0]
        apply_column_to_qiskit(FakeCircuit(), ["◦", "dec2"], 3, basis)
        self.assertEqual(basis, [1, 0, 0])

    def test_unknown_external_control_invalidates_span(self):
        basis = [None, 0, 0]
        apply_column_to_qiskit(FakeCircuit(), ["•", "dec2"], 3, basis)
        self.assertEqual(basis, [None, None, None])


def valid_token_case(token, span, decrement):
    def test(self):
        spec = parse_arithmetic_token(token)
        self.assertEqual((spec.span, spec.decrement), (span, decrement))
    return test


def rejected_case(column, qubits):
    def test(self):
        with self.assertRaises(CircuitBuildError):
            build(column, qubits)
    return test


for _span in range(1, 17):
    for _prefix in ("inc", "dec"):
        _token = f"{_prefix}{_span}"
        setattr(ArithmeticFakeTests, f"test_accept_{_token}",
                valid_token_case(_token, _span, _prefix == "dec"))

for _i, _token in enumerate(("inc", "dec", "inc0", "dec0", "inc17", "dec17",
                              "inc01", "dec+1", "inc-1", "inc1.0", "dec١", "INC1",
                              "inc2(π)", "dec1()", "inc1<a", "dec1junk")):
    setattr(ArithmeticFakeTests, f"test_reject_token_{_i}", rejected_case([_token], 16))

for _label, _column, _qubits in (
    ("overflow_inc", [1, "inc2"], 2), ("overflow_dec", ["dec4"], 3),
    ("start_outside", [1, "inc1"], 1),
    ("detect_reset_inc", ["inc2", "ZDetectControlReset"], 2),
    ("detect_reset_dec", ["dec2", "ZDetectControlReset"], 2),
):
    setattr(ArithmeticFakeTests, f"test_reject_{_label}", rejected_case(_column, _qubits))


def sparse_reference(state, column):
    """Direct integer modular permutation, never the emitted carry ladder.

    X parity is a basis change around the *odd predicate*, not a CX reduction.
    Multiple spans execute in row order after ordinary X targets.
    """
    spans = [(wire, int(token[3:]), -1 if token.startswith("dec") else 1)
             for wire, token in enumerate(column)
             if isinstance(token, str) and token.startswith(("inc", "dec"))]
    parity = [(wire, token) for wire, token in enumerate(column)
              if token in {"zpar", "xpar"}
              and not any(start <= wire < start + span for start, span, _ in spans)]
    xs = [wire for wire, token in parity if token == "xpar"]
    for wire in xs:
        state = sparse_h(state, wire)

    def enabled(index, start, span):
        external = [(wire, token) for wire, token in enumerate(column)
                    if token in {"•", "◦"} and not start <= wire < start + span]
        return (all(bool(index & (1 << wire)) == (token == "•") for wire, token in external)
                and (not parity or sum(bool(index & (1 << wire)) for wire, _ in parity) % 2 == 1))

    for wire, token in enumerate(column):
        if token == "X":
            state = {index ^ ((1 << wire) if enabled(index, wire, 1) else 0): value
                     for index, value in state.items()}
    for start, span, delta in spans:
        mask = ((1 << span) - 1) << start
        result = {}
        for index, value in state.items():
            mapped = index
            if enabled(index, start, span):
                word = (((index >> start) & ((1 << span) - 1)) + delta) % (1 << span)
                mapped = (index & ~mask) | (word << start)
            result[mapped] = value
        state = result
    for wire in reversed(xs):
        state = sparse_h(state, wire)
    return state


def sparse_h(state, wire):
    result = {}
    for index, value in state.items():
        zero, one = index & ~(1 << wire), index | (1 << wire)
        result[zero] = result.get(zero, 0j) + value / math.sqrt(2)
        result[one] = result.get(one, 0j) + (-value if index & (1 << wire) else value) / math.sqrt(2)
    return result


def sparse_primitives(state, ops):
    """Independent primitive interpreter; no arithmetic-word operations."""
    for name, *args in ops:
        if name == "h":
            state = sparse_h(state, args[0])
            continue
        if name == "x":
            controls, target = [], args[0]
        elif name == "mcx":
            controls, target = args
        elif name == "cx":
            controls, target = [args[0]], args[1]
        else:
            raise ValueError(f"unexpected primitive {name}")
        state = {index ^ ((1 << target) if all(index & (1 << c) for c in controls) else 0): value
                 for index, value in state.items()}
    return state


class ArithmeticReferenceTests(unittest.TestCase):
    pass


def exact_case(span, decrement, controls):
    def test(self):
        prefix = "dec" if decrement else "inc"
        column = list(controls) + [f"{prefix}{span}"] + [1] * (span - 1)
        qubits = len(column)
        # Unique Gaussian-integer amplitudes expose every wrong permutation,
        # wraparound, spectator bit change, and relative/global phase exactly.
        state = {i: complex(2 * i + 1, 3 * i - 2) for i in range(1 << qubits)}
        self.assertEqual(sparse_primitives(state, build(column)), sparse_reference(state, column))
    return test


for _span in range(1, 5):
    for _decrement in (False, True):
        for _i, _controls in enumerate(((), ("•",), ("◦",), ("•", "◦", "•"), ("zpar", "zpar"))):
            setattr(ArithmeticReferenceTests, f"test_exact_{_span}_{_decrement}_{_i}",
                    exact_case(_span, _decrement, _controls))


def large_sparse_case(span, decrement):
    def test(self):
        column = [f"{'dec' if decrement else 'inc'}{span}"] + [1] * (span - 1)
        state = {i: complex(i + 1, 7 - i) for i in {0, 1, 2, (1 << span) - 1, (1 << (span - 1)) - 1}}
        self.assertEqual(sparse_primitives(state, build(column)), sparse_reference(state, column))
    return test


for _span in range(1, 17):
    for _decrement in (False, True):
        setattr(ArithmeticReferenceTests, f"test_sparse_wrap_{_span}_{_decrement}",
                large_sparse_case(_span, _decrement))


def fixtures(span, prefix):
    token = f"{prefix}{span}"
    empty = [1] * (span - 1)
    return {
        "bare": [token] + empty,
        "positive": ["•", token] + empty,
        "anti": ["◦", token] + empty,
        "multiple": ["•", "◦", "•", token] + empty,
        "zpar": ["zpar", "zpar", token] + empty,
        "xpar": ["xpar", "xpar", token] + empty,
        "mixed": ["xpar", "zpar", "•", "◦", token] + empty,
        "internal": [token] + ["•", "◦", "xpar"][:span - 1],
        "internal_parity": [token] + ["xpar"] * (span - 1) + ["zpar", "xpar"],
        "two_spans": [token] + empty + ["inc2", "◦", "X"],
    }


def sparse_fixture_case(column):
    def test(self):
        state = {i: complex(i + 1, 2 * i - 3) for i in range(1 << len(column))}
        actual = sparse_primitives(state, build(column))
        expected = sparse_reference(state, column)
        self.assertLess(max(abs(actual.get(i, 0j) - expected.get(i, 0j))
                            for i in actual.keys() | expected.keys()), 1e-10)
    return test


for _span in range(1, 5):
    for _prefix in ("inc", "dec"):
        for _label, _column in fixtures(_span, _prefix).items():
            setattr(ArithmeticReferenceTests, f"test_complex_{_prefix}{_span}_{_label}",
                    sparse_fixture_case(_column))


def complex_state(qubits):
    values = {i: complex((i * 7) % 13 - 5, (i * 11) % 17 - 7) for i in range(1 << qubits)}
    norm = math.sqrt(sum(abs(v) ** 2 for v in values.values()))
    return {i: v / norm for i, v in values.items()}


class ArithmeticAerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        try:
            load_qiskit()
        except RunnerUnavailable as exc:
            raise unittest.SkipTest(str(exc))

    def statevector(self, state, columns, qubits):
        QuantumCircuit, transpile, _, AerSimulator = load_qiskit()
        qc = QuantumCircuit(qubits)
        qc.initialize([state.get(i, 0j) for i in range(1 << qubits)], range(qubits))
        apply_columns_to_qiskit(qc, columns, qubits)
        qc.save_statevector()
        simulator = AerSimulator(method="statevector", device="CPU")
        result = simulator.run(transpile(qc, simulator), shots=1).result()
        return list(result.get_statevector())


def aer_complex_case(column):
    def test(self):
        qubits = len(column)
        state = complex_state(qubits)
        expected = sparse_reference(state, column)
        actual = self.statevector(state, [column], qubits)
        self.assertLess(max(abs(a - expected.get(i, 0j)) for i, a in enumerate(actual)), 1e-10)
    return test


def inverse_case(span, prefix):
    def test(self):
        inverse = "dec" if prefix == "inc" else "inc"
        columns = [["xpar", "zpar", "◦", f"{p}{span}"] + [1] * (span - 1)
                   for p in (prefix, inverse)]
        state = complex_state(span + 3)
        actual = self.statevector(state, columns, span + 3)
        self.assertLess(max(abs(a - state[i]) for i, a in enumerate(actual)), 1e-10)
    return test


def runner_case(span, prefix, output):
    def test(self):
        qubits = span + 2
        # Nonuniform complex input, prepared solely with backend-supported gates.
        prep = [["Ry(π_3)"] * qubits, ["T"] * qubits]
        column = ["xpar", "zpar", f"{prefix}{span}"] + [1] * (span - 1)
        state = {0: 1 + 0j}
        c, s = math.cos(math.pi / 6), math.sin(math.pi / 6)
        for wire in range(qubits):
            state = {index | (bit << wire): value * factor
                     for index, value in state.items() for bit, factor in ((0, c), (1, s))}
        phase = complex(math.cos(math.pi / 4), math.sin(math.pi / 4))
        state = {i: value * phase ** i.bit_count() for i, value in state.items()}
        expected_state = sparse_reference(state, column)
        expected = [abs(expected_state.get(i, 0j)) ** 2 for i in range(1 << qubits)]
        columns = prep + [column, [f"Probability{qubits}"]]
        outputs = {"histogram": True}
        if output == "probability":
            outputs["probability"] = [{"gate_id": 9, "column": 3, "base_bit": 0,
                                       "span": qubits, "control_mask": 0, "control_value": 0}]
        result = QiskitRunner(name="qiskit-cpu-dev", device="CPU").run(parse_run_request({
            "qubits": qubits, "columns": columns, "shots": 4096, "seed": 8128, "outputs": outputs,
        }))
        if output == "histogram":
            histogram = result["histogram"]
            actual = [histogram.get(format(i, f"0{qubits}b"), 0) / 4096 for i in range(1 << qubits)]
        else:
            actual = result["probability"][0]["probabilities"]
        self.assertLess(max(abs(a - b) for a, b in zip(actual, expected)),
                        0.05 if output == "histogram" else 1e-10)
    return test


for _span in range(1, 5):
    for _prefix in ("inc", "dec"):
        for _label, _column in fixtures(_span, _prefix).items():
            setattr(ArithmeticAerTests, f"test_complex_{_prefix}{_span}_{_label}", aer_complex_case(_column))
        setattr(ArithmeticAerTests, f"test_inverse_{_prefix}{_span}", inverse_case(_span, _prefix))
        for _output in ("histogram", "probability"):
            setattr(ArithmeticAerTests, f"test_runner_{_output}_{_prefix}{_span}",
                    runner_case(_span, _prefix, _output))


if __name__ == "__main__":
    unittest.main()
