import unittest

from qni_qiskit_backend.circuit import CircuitBuildError, apply_columns_to_qiskit


class PostselectionRejectionTests(unittest.TestCase):
    def test_rejects_postselect_zero(self):
        with self.assertRaisesRegex(CircuitBuildError, "unsupported gate token"):
            apply_columns_to_qiskit(object(), [["|0⟩⟨0|"]], 1)

    def test_rejects_postselect_one(self):
        with self.assertRaisesRegex(CircuitBuildError, "unsupported gate token"):
            apply_columns_to_qiskit(object(), [["|1⟩⟨1|"]], 1)

    def test_rejects_postselect_plus(self):
        with self.assertRaisesRegex(CircuitBuildError, "unsupported gate token"):
            apply_columns_to_qiskit(object(), [["|+⟩⟨+|"]], 1)

    def test_rejects_postselect_minus(self):
        with self.assertRaisesRegex(CircuitBuildError, "unsupported gate token"):
            apply_columns_to_qiskit(object(), [["|-⟩⟨-|"]], 1)
