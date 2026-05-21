from __future__ import annotations

import cmath
import math
from collections import defaultdict
from dataclasses import dataclass
from typing import Any, Protocol, Sequence

from .circuit import apply_column_to_qiskit
from .contract import AmplitudeOutputRequest, BlochOutputRequest, RunRequest


class RunnerUnavailable(RuntimeError):
    pass


class Runner(Protocol):
    name: str

    def run(self, request: RunRequest) -> dict[str, Any]: ...


@dataclass(frozen=True)
class MockRunner:
    name: str = "mock"

    def run(self, request: RunRequest) -> dict[str, Any]:
        # Transport/UI smoke runner only. It intentionally does not simulate.
        key = "0" * request.qubits
        response = histogram_response(
            runner=self.name,
            qubits=request.qubits,
            shots=request.shots,
            histogram={key: request.shots},
        )
        add_mock_amplitudes(response, request)
        add_mock_bloch(response, request)
        return response


@dataclass(frozen=True)
class QiskitRunner:
    name: str
    device: str
    cu_state_vec: bool = False

    def run(self, request: RunRequest) -> dict[str, Any]:
        try:
            from qiskit import QuantumCircuit, transpile
            from qiskit.quantum_info import Pauli
            from qiskit_aer import AerSimulator
        except Exception as exc:  # pragma: no cover - depends on local env
            raise RunnerUnavailable(
                "qiskit and qiskit-aer are required for this runner"
            ) from exc

        qc = QuantumCircuit(request.qubits, request.qubits)
        amplitude_labels, bloch_labels = add_display_saves(qc, request, Pauli)
        for wire in range(request.qubits):
            qc.measure(wire, wire)

        options: dict[str, Any] = {"method": "statevector", "device": self.device}
        if self.cu_state_vec:
            options["cuStateVec_enable"] = True
        simulator = AerSimulator(**options)
        transpiled = transpile(qc, simulator)
        try:
            result = simulator.run(
                transpiled,
                shots=request.shots,
                seed_simulator=request.seed,
            ).result()
        except Exception as exc:  # pragma: no cover - depends on local env
            raise RunnerUnavailable(f"{self.name} execution failed: {exc}") from exc

        counts = normalize_qiskit_counts(result.get_counts())
        response = histogram_response(
            runner=self.name,
            qubits=request.qubits,
            shots=request.shots,
            histogram=counts,
        )
        data = result.data(0)
        add_qiskit_amplitudes(response, request, data, amplitude_labels)
        add_qiskit_bloch(response, request, data, bloch_labels)
        return response


def select_runner(name: str) -> Runner:
    if name == "mock":
        return MockRunner()
    if name == "qiskit-cpu-dev":
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU")
    if name == "qiskit-gpu":
        return QiskitRunner(name="qiskit-gpu", device="GPU", cu_state_vec=True)
    raise ValueError(f"unknown runner: {name}")


def add_display_saves(
    qc: Any, request: RunRequest, pauli_type: Any
) -> tuple[dict[int, str], dict[int, dict[str, str]]]:
    amplitudes_by_column: dict[int, list[AmplitudeOutputRequest]] = defaultdict(list)
    bloch_by_column: dict[int, list[BlochOutputRequest]] = defaultdict(list)
    for amplitude in request.amplitude_outputs:
        amplitudes_by_column[amplitude.column].append(amplitude)
    for bloch in request.bloch_outputs:
        bloch_by_column[bloch.column].append(bloch)
    amplitude_labels: dict[int, str] = {}
    bloch_labels: dict[int, dict[str, str]] = {}
    basis = qiskit_basis_order(request.qubits)
    for column_index, column in enumerate(request.columns):
        apply_column_to_qiskit(qc, column, request.qubits)
        for amplitude in amplitudes_by_column.get(column_index, []):
            label = f"amplitude:{amplitude.gate_id}"
            qc.save_amplitudes(basis, label=label)
            amplitude_labels[amplitude.gate_id] = label
        for bloch in bloch_by_column.get(column_index, []):
            axis_labels: dict[str, str] = {}
            for axis in ("X", "Y", "Z"):
                label = f"bloch:{bloch.gate_id}:{axis.lower()}"
                qc.save_expectation_value(pauli_type(axis), [bloch.wire], label=label)
                axis_labels[axis] = label
            bloch_labels[bloch.gate_id] = axis_labels
    return amplitude_labels, bloch_labels


def add_qiskit_amplitudes(
    response: dict[str, Any],
    request: RunRequest,
    data: dict[str, Any],
    labels: dict[int, str],
) -> None:
    if not request.amplitude_outputs:
        return
    response["amplitudes"] = [
        amplitude_display_response(
            amplitude,
            qiskit_saved_amplitudes(data[labels[amplitude.gate_id]]),
            request.qubits,
        )
        for amplitude in request.amplitude_outputs
    ]


def add_qiskit_bloch(
    response: dict[str, Any],
    request: RunRequest,
    data: dict[str, Any],
    labels: dict[int, dict[str, str]],
) -> None:
    if not request.bloch_outputs:
        return
    response["bloch"] = [
        bloch_display_response(
            bloch,
            [
                saved_float(data[labels[bloch.gate_id]["X"]]),
                saved_float(data[labels[bloch.gate_id]["Y"]]),
                saved_float(data[labels[bloch.gate_id]["Z"]]),
            ],
        )
        for bloch in request.bloch_outputs
    ]


def add_mock_amplitudes(response: dict[str, Any], request: RunRequest) -> None:
    if not request.amplitude_outputs:
        return
    state_count = 1 << request.qubits
    ground_state = [0j] * state_count
    ground_state[0] = 1 + 0j
    response["amplitudes"] = [
        amplitude_display_response(amplitude, ground_state, request.qubits)
        for amplitude in request.amplitude_outputs
    ]


def add_mock_bloch(response: dict[str, Any], request: RunRequest) -> None:
    if not request.bloch_outputs:
        return
    response["bloch"] = [
        bloch_display_response(bloch, [0.0, 0.0, 1.0])
        for bloch in request.bloch_outputs
    ]


def bloch_display_response(request: BlochOutputRequest, vector: Sequence[float]) -> dict[str, Any]:
    return {"gate_id": request.gate_id, "vector": [float(value) for value in vector[:3]]}


def saved_float(value: Any) -> float:
    return float(complex(value).real)


def qiskit_saved_amplitudes(saved: Any) -> list[complex]:
    return [complex(value) for value in list(saved)]


def qiskit_basis_order(qubits: int) -> list[int]:
    return [web_index_to_qiskit_index(index, qubits) for index in range(1 << qubits)]


def web_index_to_qiskit_index(index: int, qubits: int) -> int:
    qiskit_index = 0
    for wire in range(qubits):
        web_bit = qubits - 1 - wire
        if index & (1 << web_bit):
            qiskit_index |= 1 << wire
    return qiskit_index


def amplitude_display_response(
    request: AmplitudeOutputRequest, state: Sequence[complex], qubits: int
) -> dict[str, Any]:
    outcomes = 1 << request.span
    rest_count = 1 << (qubits - request.span)
    incoherent = [0.0] * outcomes
    best_rest = 0
    best_mag = -1.0
    incoherent_unity = 0.0

    for rest in range(rest_count):
        slice_mag = 0.0
        for outcome in range(outcomes):
            probability = abs(state_amp(state, request, rest, outcome)) ** 2
            slice_mag += probability
            incoherent[outcome] += probability
        incoherent_unity += slice_mag
        if slice_mag > best_mag:
            best_mag = slice_mag
            best_rest = rest

    raw_ket = [state_amp(state, request, best_rest, outcome) for outcome in range(outcomes)]
    unity = sum(abs(value) ** 2 for value in raw_ket)
    quality = amplitude_quality(state, request, raw_ket, unity, incoherent_unity, rest_count)
    phase_index, theta = amplitude_phase_lock(raw_ket, unity, request.phase_lock_enabled)
    normalized = normalize_amplitudes(raw_ket, unity, theta)
    incoherent_ket = normalize_incoherent(incoherent, incoherent_unity)
    return {
        "gate_id": request.gate_id,
        "span": request.span,
        "ket": [[value.real, value.imag] for value in normalized],
        "incoherent": incoherent_ket,
        "quality": quality,
        "phase_lock_index": phase_index,
    }


def state_amp(
    state: Sequence[complex], request: AmplitudeOutputRequest, rest: int, outcome: int
) -> complex:
    index = insert_outcome(rest, outcome, request.base_bit, request.span)
    if index >= len(state):
        return 0j
    if index & request.control_mask != request.control_value:
        return 0j
    return state[index]


def insert_outcome(rest: int, outcome: int, base_bit: int, span: int) -> int:
    span_mask = (1 << span) - 1
    low_mask = (1 << base_bit) - 1
    low = rest & low_mask
    high = rest & ~low_mask
    return (high << span) | ((outcome & span_mask) << base_bit) | low


def amplitude_quality(
    state: Sequence[complex],
    request: AmplitudeOutputRequest,
    raw_ket: Sequence[complex],
    unity: float,
    incoherent_unity: float,
    rest_count: int,
) -> float:
    if unity <= 1e-12 or incoherent_unity <= 1e-12:
        return 0.0
    denormalized = 0.0
    for rest in range(rest_count):
        dot_value = 0j
        for outcome, raw in enumerate(raw_ket):
            dot_value += raw.conjugate() * state_amp(state, request, rest, outcome)
        denormalized += abs(dot_value) ** 2
    return max(0.0, min(1.0, denormalized / (unity * incoherent_unity)))


def amplitude_phase_lock(
    raw_ket: Sequence[complex], unity: float, enabled: bool
) -> tuple[int, float]:
    if unity <= 1e-12 or not enabled:
        return -1, 0.0
    strongest = 0.0
    strongest_index = 0
    for outcome, raw in enumerate(raw_ket):
        probability = abs(raw) ** 2
        if probability > strongest * 10000.0:
            strongest = probability
            strongest_index = outcome
    if strongest <= 1e-8:
        return -1, 0.0
    return strongest_index, cmath.phase(raw_ket[strongest_index])


def normalize_amplitudes(raw_ket: Sequence[complex], unity: float, theta: float) -> list[complex]:
    if unity <= 1e-12:
        return [0j for _ in raw_ket]
    scale = 1 / math.sqrt(unity)
    rotation = cmath.exp(-1j * theta)
    return [value * scale * rotation for value in raw_ket]


def normalize_incoherent(probabilities: Sequence[float], unity: float) -> list[float]:
    scale = 1 / math.sqrt(max(unity, 1e-12))
    return [math.sqrt(max(value, 0.0)) * scale for value in probabilities]


def normalize_qiskit_counts(counts: dict[str, int]) -> dict[str, int]:
    # Qiskit reports classical bits high-to-low. The egui/qni editor labels
    # wires top-to-bottom, so reverse compact bitstrings for the API boundary.
    normalized: dict[str, int] = {}
    for key, value in counts.items():
        compact = key.replace(" ", "")
        normalized[compact[::-1]] = int(value)
    return dict(sorted(normalized.items()))


def histogram_response(
    *, runner: str, qubits: int, shots: int, histogram: dict[str, int]
) -> dict[str, Any]:
    return {
        "status": "completed",
        "runner": runner,
        "qubits": qubits,
        "shots": shots,
        "histogram": histogram,
        "truncated": False,
    }
