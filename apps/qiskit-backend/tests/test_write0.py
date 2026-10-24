"""Physical Write0 channels, checked with CPU Aer only as a test reference."""
import unittest

try:
    import numpy as np
    from qiskit import QuantumCircuit, transpile
    from qiskit_aer import AerSimulator
except ImportError:
    AerSimulator = None

from qni_qiskit_backend.circuit import apply_columns_to_qiskit, apply_gate


def density(circuit, qubits):
    circuit.save_density_matrix(qubits=list(range(qubits)))
    backend = AerSimulator(method="density_matrix", device="CPU")
    return np.asarray(backend.run(
        transpile(circuit, backend), shots=1, seed_simulator=132
    ).result().data(0)["density_matrix"])


def expected_reset(state, target, controls=(), antis=()):
    """K0 = I-P, K1 = X_target P, with one joint click projector."""
    no_click = np.array(state, dtype=complex)
    click = np.zeros_like(no_click)
    for index, amplitude in enumerate(state):
        if (index >> target & 1 and all(index >> c & 1 for c in controls)
                and all(not (index >> c & 1) for c in antis)):
            no_click[index] = 0
            click[index ^ (1 << target)] = amplitude
    return np.outer(no_click, no_click.conj()) + np.outer(click, click.conj())


@unittest.skipIf(AerSimulator is None, "Qiskit Aer is not installed")
class Write0Tests(unittest.TestCase):
    def test_h_then_write0_is_zero(self):
        qc = QuantumCircuit(1)
        apply_columns_to_qiskit(qc, [["H"], ["|0>"]], 1)
        np.testing.assert_allclose(density(qc, 1), [[1, 0], [0, 0]], atol=1e-12)

    def test_bell_reset_leaves_other_wire_mixed(self):
        qc = QuantumCircuit(2)
        apply_columns_to_qiskit(qc, [["H"], ["•", "X"], ["|0>"]], 2)
        np.testing.assert_allclose(density(qc, 2), np.diag([.5, 0, .5, 0]), atol=1e-12)

    def test_arbitrary_complex_entangled_input(self):
        state = np.array([1+2j, -3j, 2-1j, -4+2j]) / np.sqrt(39)
        qc = QuantumCircuit(2)
        qc.initialize(state)
        apply_gate(qc, 0, "|0>", [], [], 2, [None, None])
        np.testing.assert_allclose(density(qc, 2), expected_reset(state, 0), atol=1e-12)

    def test_quantum_control_preserves_joint_no_click_coherence(self):
        state = np.array([1, 1j, -1, 2j]) / np.sqrt(7)
        qc = QuantumCircuit(2)
        qc.initialize(state)
        apply_gate(qc, 1, "|0>", [0], [], 2, [None, None])
        np.testing.assert_allclose(density(qc, 2), expected_reset(state, 1, (0,)), atol=1e-12)

    def test_quantum_anti_controls_preserve_inactive_subspace(self):
        state = np.array([1+1j, -2j, 3, 1j, -1, 2, 3j, -2+1j]) / np.sqrt(35)
        qc = QuantumCircuit(3)
        qc.initialize(state)
        apply_gate(qc, 1, "|0>", [2], [0], 3, [None]*3)
        np.testing.assert_allclose(density(qc, 3), expected_reset(state, 1, (2,), (0,)), atol=1e-12)

    def test_quantum_control_with_known_one_target_is_not_unitary(self):
        qc = QuantumCircuit(2)
        apply_columns_to_qiskit(qc, [["H", "X"], ["•", "|0>"]], 2)
        np.testing.assert_allclose(density(qc, 2), np.diag([0, .5, .5, 0]), atol=1e-12)

    def test_known_active_controls_do_not_allocate_auxiliary(self):
        qc = QuantumCircuit(2)
        apply_columns_to_qiskit(qc, [["X", "H"], ["•", "|0>"]], 2)
        self.assertEqual(qc.num_qubits, 2)

    def test_known_active_anti_controls_do_not_allocate_auxiliary(self):
        qc = QuantumCircuit(2)
        apply_columns_to_qiskit(qc, [[1, "H"], ["◦", "|0>"]], 2)
        self.assertEqual(qc.num_qubits, 2)

    def test_shared_auxiliary_can_be_reused(self):
        qc = QuantumCircuit(3)
        apply_columns_to_qiskit(qc, [["H", "X", "X"], ["•", "|0>", "|0>"]], 3)
        np.testing.assert_allclose(density(qc, 3), np.diag([0, .5, 0, 0, 0, 0, .5, 0]), atol=1e-12)
