from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Protocol

from .circuit import apply_columns_to_qiskit
from .contract import RunRequest


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
        return histogram_response(
            runner=self.name,
            qubits=request.qubits,
            shots=request.shots,
            histogram={key: request.shots},
        )


@dataclass(frozen=True)
class QiskitRunner:
    name: str
    device: str
    cu_state_vec: bool = False

    def run(self, request: RunRequest) -> dict[str, Any]:
        try:
            from qiskit import QuantumCircuit, transpile
            from qiskit_aer import AerSimulator
        except Exception as exc:  # pragma: no cover - depends on local env
            raise RunnerUnavailable(
                "qiskit and qiskit-aer are required for this runner"
            ) from exc

        qc = QuantumCircuit(request.qubits, request.qubits)
        apply_columns_to_qiskit(qc, request.columns, request.qubits)
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
        return histogram_response(
            runner=self.name,
            qubits=request.qubits,
            shots=request.shots,
            histogram=counts,
        )


def select_runner(name: str) -> Runner:
    if name == "mock":
        return MockRunner()
    if name == "qiskit-cpu-dev":
        return QiskitRunner(name="qiskit-cpu-dev", device="CPU")
    if name == "qiskit-gpu":
        return QiskitRunner(name="qiskit-gpu", device="GPU", cu_state_vec=True)
    raise ValueError(f"unknown runner: {name}")


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
