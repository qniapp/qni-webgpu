import unittest

from qni_qiskit_backend.contract import ContractError, parse_run_request
from qni_qiskit_backend.runners import MockRunner, QiskitRunner, normalize_qiskit_counts, select_runner


class ContractTests(unittest.TestCase):
    def test_rejects_full_statevector_output(self):
        with self.assertRaises(ContractError):
            parse_run_request(
                {
                    "qubits": 2,
                    "columns": [],
                    "outputs": {"histogram": True, "statevector": True},
                }
            )

    def test_mock_runner_returns_histogram_only(self):
        request = parse_run_request({"qubits": 3, "columns": [], "shots": 12})
        response = MockRunner().run(request)
        self.assertEqual(response["histogram"], {"000": 12})
        self.assertNotIn("statevector", response)
        self.assertNotIn("probabilities", response)

    def test_cpu_dev_runner_is_explicit_not_fallback(self):
        runner = select_runner("qiskit-cpu-dev")
        self.assertIsInstance(runner, QiskitRunner)
        self.assertEqual(runner.device, "CPU")
        self.assertFalse(runner.cu_state_vec)

    def test_gpu_runner_requires_gpu_options(self):
        runner = select_runner("qiskit-gpu")
        self.assertIsInstance(runner, QiskitRunner)
        self.assertEqual(runner.device, "GPU")
        self.assertTrue(runner.cu_state_vec)

    def test_qiskit_counts_are_reoriented_to_wire_order(self):
        self.assertEqual(normalize_qiskit_counts({"10": 5, "01": 7}), {"01": 5, "10": 7})


if __name__ == "__main__":
    unittest.main()
