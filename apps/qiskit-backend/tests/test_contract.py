import math
import unittest

from qni_qiskit_backend.circuit import CircuitBuildError, apply_columns_to_qiskit
from qni_qiskit_backend.contract import ContractError, parse_run_request
from qni_qiskit_backend.runners import (
    MockRunner,
    QiskitRunner,
    add_display_saves,
    add_qiskit_density,
    add_qiskit_probability,
    amplitude_display_response,
    density_qargs,
    normalize_qiskit_counts,
    probability_qargs,
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

    def test_contract_accepts_probability_output_requests(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["H"], ["Probability"]],
                "outputs": {
                    "histogram": True,
                    "probability": [{"gate_id": 2, "column": 1, "span": 1, "base_bit": 0}],
                },
            }
        )
        self.assertEqual(request.probability_outputs[0].gate_id, 2)

    def test_contract_rejects_too_many_probability_outputs(self):
        probability = [
            {"gate_id": index, "column": 0, "span": 1, "base_bit": 0}
            for index in range(33)
        ]
        with self.assertRaises(ContractError):
            parse_run_request(
                {
                    "qubits": 1,
                    "columns": [["Probability"]],
                    "outputs": {"histogram": True, "probability": probability},
                }
            )

    def test_contract_accepts_density_output_requests(self):
        request = parse_run_request(
            {
                "qubits": 2,
                "columns": [["H", "H"], ["Density2"]],
                "outputs": {
                    "histogram": True,
                    "densities": [{"gate_id": 2, "column": 1, "span": 2, "base_bit": 0}],
                },
            }
        )
        self.assertEqual(request.density_outputs[0].gate_id, 2)

    def test_contract_rejects_density_span_above_quirk_limit(self):
        with self.assertRaises(ContractError):
            parse_run_request(
                {
                    "qubits": 9,
                    "columns": [["Density9"]],
                    "outputs": {
                        "histogram": True,
                        "densities": [{"gate_id": 1, "column": 0, "span": 9, "base_bit": 0}],
                    },
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

    def test_qiskit_builder_ignores_probability_display_tokens(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["Probability2"]], 2)
        self.assertEqual(fake.ops, [])

    def test_qiskit_builder_ignores_density_display_tokens(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["Density2"]], 2)
        self.assertEqual(fake.ops, [])

    def test_qiskit_builder_applies_write0_to_deterministic_one(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

            def x(self, wire):
                self.ops.append(("x", wire))

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["X"], ["|0>"]], 1)
        self.assertEqual(fake.ops, [("x", 0), ("x", 0)])

    def test_qiskit_builder_rejects_write0_after_superposition(self):
        class FakeCircuit:
            def h(self, _wire):
                pass

        with self.assertRaises(CircuitBuildError):
            apply_columns_to_qiskit(FakeCircuit(), [["H"], ["|0>"]], 1)

    def test_qiskit_builder_applies_write1_to_deterministic_zero(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

            def x(self, wire):
                self.ops.append(("x", wire))

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["|1>"]], 1)
        self.assertEqual(fake.ops, [("x", 0)])

    def test_qiskit_builder_rejects_write1_after_superposition(self):
        class FakeCircuit:
            def h(self, _wire):
                pass

        with self.assertRaises(CircuitBuildError):
            apply_columns_to_qiskit(FakeCircuit(), [["H"], ["|1>"]], 1)

    def test_qiskit_display_saves_tracks_write0_across_columns(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

            def x(self, wire):
                self.ops.append(("x", wire))

        fake = FakeCircuit()
        request = parse_run_request({"qubits": 1, "columns": [["X"], ["|0>"]]})
        add_display_saves(fake, request, lambda axis: axis)
        self.assertEqual(fake.ops, [("x", 0), ("x", 0)])

    def test_qiskit_display_saves_rejects_write0_after_superposition(self):
        class FakeCircuit:
            def h(self, _wire):
                pass

        request = parse_run_request({"qubits": 1, "columns": [["H"], ["|0>"]]})
        with self.assertRaises(CircuitBuildError):
            add_display_saves(FakeCircuit(), request, lambda axis: axis)

    def test_qiskit_display_saves_tracks_write1_across_columns(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

            def x(self, wire):
                self.ops.append(("x", wire))

        fake = FakeCircuit()
        request = parse_run_request({"qubits": 1, "columns": [["|1>"], ["|1>"]]})
        add_display_saves(fake, request, lambda axis: axis)
        self.assertEqual(fake.ops, [("x", 0)])

    def test_qiskit_display_saves_rejects_write1_after_superposition(self):
        class FakeCircuit:
            def h(self, _wire):
                pass

        request = parse_run_request({"qubits": 1, "columns": [["H"], ["|1>"]]})
        with self.assertRaises(CircuitBuildError):
            add_display_saves(FakeCircuit(), request, lambda axis: axis)

    def test_qiskit_builder_expands_qft2_token(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

            def h(self, wire):
                self.ops.append(("h", wire))

            def cp(self, phase, control, target):
                self.ops.append(("cp", phase, control, target))

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["QFT2", 1]], 2)
        self.assertEqual(fake.ops, [("h", 0), ("cp", math.pi / 2, 1, 0), ("h", 1)])

    def test_qiskit_builder_expands_qft_dagger2_token(self):
        class FakeCircuit:
            def __init__(self):
                self.ops = []

            def h(self, wire):
                self.ops.append(("h", wire))

            def cp(self, phase, control, target):
                self.ops.append(("cp", phase, control, target))

        fake = FakeCircuit()
        apply_columns_to_qiskit(fake, [["QFT†2", 1]], 2)
        self.assertEqual(fake.ops, [("h", 1), ("cp", -math.pi / 2, 1, 0), ("h", 0)])

    def test_qiskit_display_saves_tracks_qft_as_unknown_basis(self):
        class FakeCircuit:
            def h(self, _wire):
                pass

            def cp(self, _phase, _control, _target):
                pass

        request = parse_run_request({"qubits": 2, "columns": [["QFT2", 1], ["|1>", 1]]})
        with self.assertRaises(CircuitBuildError):
            add_display_saves(FakeCircuit(), request, lambda axis: axis)

    def test_qiskit_display_saves_probability_with_web_order_qargs(self):
        class FakeCircuit:
            def __init__(self):
                self.saved = []

            def save_probabilities(self, qubits, label):
                self.saved.append((qubits, label))

        fake = FakeCircuit()
        request = parse_run_request(
            {
                "qubits": 2,
                "columns": [["Probability2"]],
                "outputs": {
                    "histogram": True,
                    "probability": [{"gate_id": 1, "column": 0, "span": 2, "base_bit": 0}],
                },
            }
        )
        labels = add_display_saves(fake, request, lambda axis: axis)
        self.assertEqual((fake.saved, labels[2]), ([([1, 0], "probability:1")], {1: "probability:1"}))

    def test_qiskit_display_saves_density_with_web_order_qargs(self):
        class FakeCircuit:
            def __init__(self):
                self.saved = []

            def save_density_matrix(self, qubits, label):
                self.saved.append((qubits, label))

        fake = FakeCircuit()
        request = parse_run_request(
            {
                "qubits": 2,
                "columns": [["Density2"]],
                "outputs": {
                    "histogram": True,
                    "densities": [{"gate_id": 1, "column": 0, "span": 2, "base_bit": 0}],
                },
            }
        )
        labels = add_display_saves(fake, request, lambda axis: axis)
        self.assertEqual((fake.saved, labels[3]), ([([1, 0], "density:1")], {1: "density:1"}))

    def test_qiskit_probability_response_uses_saved_data(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Probability"]],
                "outputs": {
                    "histogram": True,
                    "probability": [{"gate_id": 1, "column": 0, "span": 1, "base_bit": 0}],
                },
            }
        )
        response = {}
        add_qiskit_probability(response, request, {"probability:1": [0.25, 0.75]}, {1: "probability:1"})
        self.assertEqual(response["probability"][0]["probabilities"][1], 0.75)

    def test_qiskit_density_response_uses_saved_data(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Density"]],
                "outputs": {
                    "histogram": True,
                    "densities": [{"gate_id": 1, "column": 0, "span": 1, "base_bit": 0}],
                },
            }
        )
        response = {}
        add_qiskit_density(response, request, {"density:1": [[0.5, 0.25j], [-0.25j, 0.5]]}, {1: "density:1"})
        self.assertEqual(response["densities"][0]["cells"][1], [0.0, 0.25])

    def test_probability_qargs_match_web_outcome_order(self):
        request = parse_run_request(
            {
                "qubits": 2,
                "columns": [["Probability2"]],
                "outputs": {
                    "histogram": True,
                    "probability": [{"gate_id": 1, "column": 0, "span": 2, "base_bit": 0}],
                },
            }
        )
        self.assertEqual(probability_qargs(request.probability_outputs[0], 2), [1, 0])

    def test_density_qargs_match_web_outcome_order(self):
        request = parse_run_request(
            {
                "qubits": 2,
                "columns": [["Density2"]],
                "outputs": {
                    "histogram": True,
                    "densities": [{"gate_id": 1, "column": 0, "span": 2, "base_bit": 0}],
                },
            }
        )
        self.assertEqual(density_qargs(request.density_outputs[0], 2), [1, 0])

    def test_mock_runner_returns_fixed_probability_outputs(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Probability"]],
                "outputs": {
                    "histogram": True,
                    "probability": [{"gate_id": 1, "column": 0, "span": 1, "base_bit": 0}],
                },
            }
        )
        response = MockRunner().run(request)
        self.assertEqual(response["probability"][0]["probabilities"], [1.0, 0.0])

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

    def test_mock_runner_returns_fixed_density_outputs(self):
        request = parse_run_request(
            {
                "qubits": 1,
                "columns": [["Density"]],
                "outputs": {
                    "histogram": True,
                    "densities": [{"gate_id": 1, "column": 0, "span": 1, "base_bit": 0}],
                },
            }
        )
        response = MockRunner().run(request)
        self.assertEqual(response["densities"][0]["cells"][0], [1.0, 0.0])

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
