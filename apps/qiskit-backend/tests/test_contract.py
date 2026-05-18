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

    def test_accepts_thirty_two_qubits(self):
        request = parse_run_request({"qubits": 32, "columns": [], "shots": 12})
        self.assertEqual(request.qubits, 32)

    def test_rejects_more_than_thirty_two_qubits(self):
        with self.assertRaises(ContractError):
            parse_run_request({"qubits": 33, "columns": [], "shots": 12})

    def test_mock_runner_returns_histogram_only(self):
        request = parse_run_request({"qubits": 3, "columns": [], "shots": 12})
        response = MockRunner().run(request)
        self.assertEqual(
            {
                "histogram": response["histogram"],
                "has_statevector": "statevector" in response,
                "has_probabilities": "probabilities" in response,
            },
            {"histogram": {"000": 12}, "has_statevector": False, "has_probabilities": False},
        )

    def test_cpu_dev_runner_is_explicit_not_fallback(self):
        runner = select_runner("qiskit-cpu-dev")
        self.assertEqual(
            (isinstance(runner, QiskitRunner), runner.device, runner.cu_state_vec),
            (True, "CPU", False),
        )

    def test_gpu_runner_requires_gpu_options(self):
        runner = select_runner("qiskit-gpu")
        self.assertEqual(
            (isinstance(runner, QiskitRunner), runner.device, runner.cu_state_vec),
            (True, "GPU", True),
        )

    def test_qiskit_counts_are_reoriented_to_wire_order(self):
        self.assertEqual(normalize_qiskit_counts({"10": 5, "01": 7}), {"01": 5, "10": 7})


if __name__ == "__main__":
    unittest.main()
