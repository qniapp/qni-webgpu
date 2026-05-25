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
MAX_BLOCH_OUTPUTS = 64
MAX_PROBABILITY_OUTPUTS = 32
MAX_DENSITY_OUTPUTS = 16
MAX_DENSITY_SPAN = 8
RUNNERS = frozenset({"mock", "qiskit-cpu-dev", "qiskit-gpu"})


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
class BlochOutputRequest:
    gate_id: int
    column: int
    wire: int


@dataclass(frozen=True)
class ProbabilityOutputRequest:
    gate_id: int
    column: int
    span: int
    base_bit: int


@dataclass(frozen=True)
class DensityOutputRequest:
    gate_id: int
    column: int
    span: int
    base_bit: int
    control_mask: int
    control_value: int


@dataclass(frozen=True)
class RunRequest:
    runner: str
    qubits: int
    columns: list[list[Any]]
    shots: int
    seed: int | None
    amplitude_outputs: list[AmplitudeOutputRequest]
    bloch_outputs: list[BlochOutputRequest]
    probability_outputs: list[ProbabilityOutputRequest]
    density_outputs: list[DensityOutputRequest]


def parse_run_request(
    payload: dict[str, Any],
    default_runner: str = "mock",
    allowed_runners: set[str] | frozenset[str] | None = None,
) -> RunRequest:
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
    bloch_outputs = parse_bloch_outputs(outputs, columns, qubits)
    probability_outputs = parse_probability_outputs(outputs, columns, qubits)
    density_outputs = parse_density_outputs(outputs, columns, qubits)

    allowed = RUNNERS if allowed_runners is None else frozenset(allowed_runners)
    if not allowed:
        raise ContractError("at least one runner must be allowed")
    unknown_allowed = sorted(allowed - RUNNERS)
    if unknown_allowed:
        raise ContractError(f"unknown allowed runner: {', '.join(unknown_allowed)}")

    runner = payload.get("runner", default_runner)
    if runner not in RUNNERS:
        raise ContractError("runner must be mock, qiskit-cpu-dev, or qiskit-gpu")
    if runner not in allowed:
        raise ContractError(f"runner is not allowed by this server: {runner}")

    return RunRequest(
        runner=runner,
        qubits=qubits,
        columns=columns,
        shots=shots,
        seed=seed,
        amplitude_outputs=amplitude_outputs,
        bloch_outputs=bloch_outputs,
        probability_outputs=probability_outputs,
        density_outputs=density_outputs,
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
    gate_id = required_int(raw, "gate_id", "amplitude")
    column = required_int(raw, "column", "amplitude")
    span = required_int(raw, "span", "amplitude")
    base_bit = required_int(raw, "base_bit", "amplitude")
    control_mask = required_int(raw, "control_mask", "amplitude")
    control_value = required_int(raw, "control_value", "amplitude")
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


def parse_bloch_outputs(
    outputs: dict[str, Any], columns: list[list[Any]], qubits: int
) -> list[BlochOutputRequest]:
    raw_outputs = outputs.get("bloch", [])
    if raw_outputs in (None, False):
        return []
    if not isinstance(raw_outputs, list):
        raise ContractError("outputs.bloch must be an array")
    if len(raw_outputs) > MAX_BLOCH_OUTPUTS:
        raise ContractError(f"at most {MAX_BLOCH_OUTPUTS} bloch outputs are supported")
    return [parse_bloch_output(raw, columns, qubits) for raw in raw_outputs]


def parse_bloch_output(raw: Any, columns: list[list[Any]], qubits: int) -> BlochOutputRequest:
    if not isinstance(raw, dict):
        raise ContractError("each bloch output must be an object")
    gate_id = required_int(raw, "gate_id", "bloch")
    column = required_int(raw, "column", "bloch")
    wire = required_int(raw, "wire", "bloch")
    if gate_id < 0:
        raise ContractError("bloch gate_id must be non-negative")
    if not 0 <= column < len(columns):
        raise ContractError("bloch column is out of range")
    if not 0 <= wire < qubits:
        raise ContractError("bloch wire is out of range")
    return BlochOutputRequest(gate_id=gate_id, column=column, wire=wire)


def parse_probability_outputs(
    outputs: dict[str, Any], columns: list[list[Any]], qubits: int
) -> list[ProbabilityOutputRequest]:
    raw_outputs = outputs.get("probability", [])
    if raw_outputs in (None, False):
        return []
    if not isinstance(raw_outputs, list):
        raise ContractError("outputs.probability must be an array")
    if len(raw_outputs) > MAX_PROBABILITY_OUTPUTS:
        raise ContractError(f"at most {MAX_PROBABILITY_OUTPUTS} probability outputs are supported")
    return [parse_probability_output(raw, columns, qubits) for raw in raw_outputs]


def parse_probability_output(
    raw: Any, columns: list[list[Any]], qubits: int
) -> ProbabilityOutputRequest:
    if not isinstance(raw, dict):
        raise ContractError("each probability output must be an object")
    gate_id = required_int(raw, "gate_id", "probability")
    column = required_int(raw, "column", "probability")
    span = required_int(raw, "span", "probability")
    base_bit = required_int(raw, "base_bit", "probability")
    if gate_id < 0:
        raise ContractError("probability gate_id must be non-negative")
    if not 0 <= column < len(columns):
        raise ContractError("probability column is out of range")
    if not 1 <= span <= 16:
        raise ContractError("probability span must be in [1, 16]")
    if not 0 <= base_bit or base_bit + span > qubits:
        raise ContractError("probability bit range is out of range")
    return ProbabilityOutputRequest(
        gate_id=gate_id,
        column=column,
        span=span,
        base_bit=base_bit,
    )


def parse_density_outputs(
    outputs: dict[str, Any], columns: list[list[Any]], qubits: int
) -> list[DensityOutputRequest]:
    raw_outputs = outputs.get("densities", [])
    if raw_outputs in (None, False):
        return []
    if not isinstance(raw_outputs, list):
        raise ContractError("outputs.densities must be an array")
    if len(raw_outputs) > MAX_DENSITY_OUTPUTS:
        raise ContractError(f"at most {MAX_DENSITY_OUTPUTS} density outputs are supported")
    return [parse_density_output(raw, columns, qubits) for raw in raw_outputs]


def parse_density_output(
    raw: Any, columns: list[list[Any]], qubits: int
) -> DensityOutputRequest:
    if not isinstance(raw, dict):
        raise ContractError("each density output must be an object")
    gate_id = required_int(raw, "gate_id", "density")
    column = required_int(raw, "column", "density")
    span = required_int(raw, "span", "density")
    base_bit = required_int(raw, "base_bit", "density")
    control_mask = optional_int(raw, "control_mask", "density", default=0)
    control_value = optional_int(raw, "control_value", "density", default=0)
    if gate_id < 0:
        raise ContractError("density gate_id must be non-negative")
    if not 0 <= column < len(columns):
        raise ContractError("density column is out of range")
    if not 1 <= span <= MAX_DENSITY_SPAN:
        raise ContractError(f"density span must be in [1, {MAX_DENSITY_SPAN}]")
    if not 0 <= base_bit or base_bit + span > qubits:
        raise ContractError("density bit range is out of range")
    max_mask = (1 << qubits) - 1
    if not 0 <= control_mask <= max_mask:
        raise ContractError("density control_mask is out of range")
    if not 0 <= control_value <= max_mask:
        raise ContractError("density control_value is out of range")
    if control_value & ~control_mask:
        raise ContractError("density control_value must be a subset of control_mask")
    display_bits = set(range(base_bit, base_bit + span))
    non_display_control_count = sum(
        1 for bit in range(qubits) if control_mask & (1 << bit) and bit not in display_bits
    )
    if span + non_display_control_count > MAX_DENSITY_SPAN:
        raise ContractError(
            f"density span plus non-display controls must be in [1, {MAX_DENSITY_SPAN}]"
        )
    return DensityOutputRequest(
        gate_id=gate_id,
        column=column,
        span=span,
        base_bit=base_bit,
        control_mask=control_mask,
        control_value=control_value,
    )


def optional_int(raw: dict[str, Any], key: str, subject: str, *, default: int) -> int:
    if key not in raw:
        return default
    return required_int(raw, key, subject)


def required_int(raw: dict[str, Any], key: str, subject: str) -> int:
    value = raw.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        raise ContractError(f"{subject} {key} must be an integer")
    return value
