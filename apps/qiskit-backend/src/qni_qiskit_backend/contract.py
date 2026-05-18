from __future__ import annotations

from dataclasses import dataclass
from typing import Any

FORBIDDEN_OUTPUTS = {
    "statevector",
    "stateVector",
    "full_statevector",
    "fullStateVector",
    "probabilities",
    "full_probabilities",
    "fullProbabilities",
}
MAX_SHOTS = 100_000
MAX_QUBITS = 32


class ContractError(ValueError):
    pass


@dataclass(frozen=True)
class RunRequest:
    runner: str
    qubits: int
    columns: list[list[Any]]
    shots: int
    seed: int | None


def parse_run_request(payload: dict[str, Any], default_runner: str = "mock") -> RunRequest:
    outputs = payload.get("outputs", {"histogram": True})
    if not isinstance(outputs, dict):
        raise ContractError("outputs must be an object")
    forbidden = sorted(name for name in outputs if name in FORBIDDEN_OUTPUTS and outputs[name])
    if forbidden:
        raise ContractError(f"forbidden full-vector output requested: {', '.join(forbidden)}")
    if outputs.get("histogram") is False:
        raise ContractError("histogram output is required in this phase")

    qubits = payload.get("qubits")
    if not isinstance(qubits, int) or not 1 <= qubits <= MAX_QUBITS:
        raise ContractError(f"qubits must be an integer in [1, {MAX_QUBITS}]")

    columns = payload.get("columns")
    if not isinstance(columns, list):
        raise ContractError("columns must be an array")
    for column in columns:
        if not isinstance(column, list):
            raise ContractError("each column must be an array")
        if len(column) > qubits:
            raise ContractError("column references a wire beyond qubits")

    shots = payload.get("shots", 1024)
    if not isinstance(shots, int) or not 1 <= shots <= MAX_SHOTS:
        raise ContractError(f"shots must be an integer in [1, {MAX_SHOTS}]")

    seed = payload.get("seed")
    if seed is not None and not isinstance(seed, int):
        raise ContractError("seed must be an integer when provided")

    runner = payload.get("runner", default_runner)
    if runner not in {"mock", "qiskit-cpu-dev", "qiskit-gpu"}:
        raise ContractError("runner must be mock, qiskit-cpu-dev, or qiskit-gpu")

    return RunRequest(runner=runner, qubits=qubits, columns=columns, shots=shots, seed=seed)
