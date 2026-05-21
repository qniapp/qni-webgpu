import unittest

from qni_qiskit_backend.circuit import apply_columns_to_qiskit
from qni_qiskit_backend.contract import ContractError, parse_run_request
from qni_qiskit_backend.runners import (
    MockRunner,
    QiskitRunner,
    amplitude_display_response,
    normalize_qiskit_counts,
    select_runner,
)


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

    def test_mock_runner_returns_histogram_only_without_amplitude_requests(self):
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

    def test_contract_accepts_amplitude_output_requests(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["H"], ["Amps1"]],
                "outputs": {
                    "histogram": True,
                    "amplitudes": [
                        {
                            "gate_id": 2,
                            "column": 1,
                            "span": 1,
                            "base_bit": 0,
                            "control_mask": 0,
                            "control_value": 0,
                            "phase_lock_enabled": False,
                        }
                    ],
                },
            }
        )
        self.assertEqual(request.amplitude_outputs[0].gate_id, 2)

    def test_contract_rejects_large_exact_amplitude_outputs(self):
        with self.assertRaises(ContractError):
            parse_run_request(
                {
                    "qubits": 17,
                    "columns": [["Amps1"]],
                    "outputs": {
                        "histogram": True,
                        "amplitudes": [
                            {
                                "gate_id": 1,
                                "column": 0,
                                "span": 1,
                                "base_bit": 0,
                                "control_mask": 0,
                                "control_value": 0,
                                "phase_lock_enabled": True,
                            }
                        ],
                    },
                }
            )

    def test_contract_rejects_too_many_amplitude_outputs(self):
        amplitudes = [
            {
                "gate_id": index,
                "column": 0,
                "span": 1,
                "base_bit": 0,
                "control_mask": 0,
                "control_value": 0,
                "phase_lock_enabled": False,
            }
            for index in range(33)
        ]
        with self.assertRaises(ContractError):
            parse_run_request(
                {
                    "qubits": 1,
                    "columns": [["Amps1"]],
                    "outputs": {"histogram": True, "amplitudes": amplitudes},
                }
            )

    def test_contract_accepts_bloch_output_requests(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["H"], ["Bloch"]],
                "outputs": {
                    "histogram": True,
                    "bloch": [{"gate_id": 2, "column": 1, "wire": 0}],
                },
            }
        )
        self.assertEqual(request.bloch_outputs[0].gate_id, 2)

    def test_contract_rejects_too_many_bloch_outputs(self):
        bloch = [{"gate_id": index, "column": 0, "wire": 0} for index in range(65)]
        with self.assertRaises(ContractError):
            parse_run_request(
                {
                    "qubits": 1,
                    "columns": [["Bloch"]],
                    "outputs": {"histogram": True, "bloch": bloch},
                }
            )

    def test_qiskit_builder_ignores_amplitude_display_tokens(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["Amps1"]], 1)
        self.assertEqual(fake.ops, [])

    def test_qiskit_builder_ignores_bloch_display_tokens(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["Bloch"]], 1)
        self.assertEqual(fake.ops, [])

    def test_mock_runner_returns_fixed_bloch_outputs(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Bloch"]],
                "outputs": {
                    "histogram": True,
                    "bloch": [{"gate_id": 1, "column": 0, "wire": 0}],
                },
            }
        )
        response = MockRunner().run(request)
        self.assertEqual(response["bloch"][0]["vector"], [0.0, 0.0, 1.0])

    def test_mock_runner_returns_fixed_amplitude_outputs(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Amps1"]],
                "outputs": {
                    "histogram": True,
                    "amplitudes": [
                        {
                            "gate_id": 1,
                            "column": 0,
                            "span": 1,
                            "base_bit": 0,
                            "control_mask": 0,
                            "control_value": 0,
                            "phase_lock_enabled": False,
                        }
                    ],
                },
            }
        )
        response = MockRunner().run(request)
        self.assertEqual(response["amplitudes"][0]["ket"], [[1.0, 0.0], [0.0, 0.0]])

    def test_amplitude_response_matches_h_state(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Amps1"]],
                "outputs": {
                    "histogram": True,
                    "amplitudes": [
                        {
                            "gate_id": 1,
                            "column": 0,
                            "span": 1,
                            "base_bit": 0,
                            "control_mask": 0,
                            "control_value": 0,
                            "phase_lock_enabled": False,
                        }
                    ],
                },
            }
        )
        response = amplitude_display_response(request.amplitude_outputs[0], [2**-0.5, 2**-0.5], 1)
        self.assertEqual(round(response["ket"][1][0], 3), 0.707)

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
