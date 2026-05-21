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
MAX_EXACT_AMPLITUDE_QUBITS = 16
MAX_AMPLITUDE_OUTPUTS = 32


class ContractError(ValueError):
    pass


@dataclass(frozen=True)
class AmplitudeOutputRequest:
    gate_id: int
    column: int
    span: int
    base_bit: int
    control_mask: int
    control_value: int
    phase_lock_enabled: bool


@dataclass(frozen=True)
class RunRequest:
    runner: str
    qubits: int
    columns: list[list[Any]]
    shots: int
    seed: int | None
    amplitude_outputs: list[AmplitudeOutputRequest]


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

    amplitude_outputs = parse_amplitude_outputs(outputs, columns, qubits)

    runner = payload.get("runner", default_runner)
    if runner not in {"mock", "qiskit-cpu-dev", "qiskit-gpu"}:
        raise ContractError("runner must be mock, qiskit-cpu-dev, or qiskit-gpu")

    return RunRequest(
        runner=runner,
        qubits=qubits,
        columns=columns,
        shots=shots,
        seed=seed,
        amplitude_outputs=amplitude_outputs,
    )


def parse_amplitude_outputs(
    outputs: dict[str, Any], columns: list[list[Any]], qubits: int
) -> list[AmplitudeOutputRequest]:
    raw_outputs = outputs.get("amplitudes", [])
    if raw_outputs in (None, False):
        return []
    if not isinstance(raw_outputs, list):
        raise ContractError("outputs.amplitudes must be an array")
    if raw_outputs and qubits > MAX_EXACT_AMPLITUDE_QUBITS:
        raise ContractError(
            f"amplitude outputs require qubits <= {MAX_EXACT_AMPLITUDE_QUBITS} in this phase"
        )
    if len(raw_outputs) > MAX_AMPLITUDE_OUTPUTS:
        raise ContractError(f"at most {MAX_AMPLITUDE_OUTPUTS} amplitude outputs are supported")
    return [parse_amplitude_output(raw, columns, qubits) for raw in raw_outputs]


def parse_amplitude_output(
    raw: Any, columns: list[list[Any]], qubits: int
) -> AmplitudeOutputRequest:
    if not isinstance(raw, dict):
        raise ContractError("each amplitude output must be an object")
    gate_id = required_int(raw, "gate_id")
    column = required_int(raw, "column")
    span = required_int(raw, "span")
    base_bit = required_int(raw, "base_bit")
    control_mask = required_int(raw, "control_mask")
    control_value = required_int(raw, "control_value")
    phase_lock_enabled = raw.get("phase_lock_enabled")
    if not isinstance(phase_lock_enabled, bool):
        raise ContractError("amplitude phase_lock_enabled must be a boolean")
    if gate_id < 0:
        raise ContractError("amplitude gate_id must be non-negative")
    if not 0 <= column < len(columns):
        raise ContractError("amplitude column is out of range")
    if not 1 <= span <= 16:
        raise ContractError("amplitude span must be in [1, 16]")
    if not 0 <= base_bit or base_bit + span > qubits:
        raise ContractError("amplitude bit range is out of range")
    max_mask = (1 << qubits) - 1
    if not 0 <= control_mask <= max_mask:
        raise ContractError("amplitude control_mask is out of range")
    if not 0 <= control_value <= max_mask:
        raise ContractError("amplitude control_value is out of range")
    if control_value & ~control_mask:
        raise ContractError("amplitude control_value must be a subset of control_mask")
    return AmplitudeOutputRequest(
        gate_id=gate_id,
        column=column,
        span=span,
        base_bit=base_bit,
        control_mask=control_mask,
        control_value=control_value,
        phase_lock_enabled=phase_lock_enabled,
    )


def required_int(raw: dict[str, Any], key: str) -> int:
    value = raw.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        raise ContractError(f"amplitude {key} must be an integer")
    return value
