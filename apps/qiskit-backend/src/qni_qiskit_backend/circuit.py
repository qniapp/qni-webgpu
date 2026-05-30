from __future__ import annotations

import ast
import math
import re
from dataclasses import dataclass
from typing import Any

CONTROL_TOKENS = {"•", "●", "control", "Control"}
ANTI_CONTROL_TOKENS = {"◦", "○", "anti", "Anti"}
EMPTY_TOKENS = {None, 1, "1", ""}
AMPLITUDE_DISPLAY_RE = re.compile(r"^Amps(?:[1-9]|1[0-6])$")
PROBABILITY_DISPLAY_RE = re.compile(r"^Probability(?:[1-9]|1[0-6])?$")
DENSITY_DISPLAY_RE = re.compile(r"^Density(?:[1-8])?$")
DISPLAY_TOKENS = {"Bloch"}


class CircuitBuildError(ValueError):
    pass


def token_text(token: Any) -> str | None:
    if token in EMPTY_TOKENS:
        return None
    return str(token)


def parse_angle(expr: str | None, default: float = math.pi / 2) -> float:
    if expr is None:
        return default
    if not expr.strip():
        raise CircuitBuildError("unsupported angle expression")
    normalized = normalize_angle_expr(expr)
    try:
        tree = ast.parse(normalized, mode="eval")
    except SyntaxError as exc:
        raise CircuitBuildError("unsupported angle expression") from exc
    return float(_eval_angle_node(tree.body))


def normalize_angle_expr(expr: str) -> str:
    normalized = expr.strip().replace("_", "/").replace("π", "pi")
    return re.sub(r"(?<=\d)pi\b", "*pi", normalized)


def _eval_angle_node(node: ast.AST) -> float:
    if isinstance(node, ast.Constant) and isinstance(node.value, (int, float)):
        return float(node.value)
    if isinstance(node, ast.Name) and node.id == "pi":
        return math.pi
    if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
        return -_eval_angle_node(node.operand)
    if isinstance(node, ast.BinOp):
        left = _eval_angle_node(node.left)
        right = _eval_angle_node(node.right)
        if isinstance(node.op, ast.Add):
            return left + right
        if isinstance(node.op, ast.Sub):
            return left - right
        if isinstance(node.op, ast.Mult):
            return left * right
        if isinstance(node.op, ast.Div):
            if right == 0:
                raise CircuitBuildError("unsupported angle expression")
            return left / right
    raise CircuitBuildError("unsupported angle expression")


def split_parametric(token: str) -> tuple[str, str | None]:
    match = re.fullmatch(r"([^()]+)(?:\((.*)\))?", token)
    if not match:
        return token, None
    base, angle = match.groups()
    return base, angle.replace("_", "/") if angle is not None else None


BasisTracker = list[int | None]


@dataclass(frozen=True)
class QftSpec:
    span: int
    dagger: bool


def apply_columns_to_qiskit(qc: Any, columns: list[list[Any]], qubits: int) -> None:
    basis = initial_basis_tracker(qubits)
    for column in columns:
        apply_column_to_qiskit(qc, column, qubits, basis)


def initial_basis_tracker(qubits: int) -> BasisTracker:
    return [0 for _ in range(qubits)]


def apply_column_to_qiskit(
    qc: Any, column: list[Any], qubits: int, basis: BasisTracker
) -> None:
    controls: list[int] = []
    anti_controls: list[int] = []
    swap_wires: list[int] = []
    measurement_wires: list[int] = []
    has_readonly_display = False
    deferred: list[tuple[int, str]] = []
    for wire, raw in enumerate(column):
        token = token_text(raw)
        if token is None:
            continue
        if token in CONTROL_TOKENS:
            controls.append(wire)
            continue
        if token in ANTI_CONTROL_TOKENS:
            anti_controls.append(wire)
            continue
        if token == "Swap":
            swap_wires.append(wire)
            continue
        if token == "…":
            continue
        if token == "Measure":
            measurement_wires.append(wire)
            continue
        if is_readonly_display_token(token):
            has_readonly_display = True
            continue
        deferred.append((wire, token))
    apply_swap(qc, swap_wires, controls, anti_controls, basis)
    for wire, token in deferred:
        apply_gate(qc, wire, token, controls, anti_controls, qubits, basis)
    apply_control_only_z(
        qc,
        controls,
        anti_controls,
        basis,
        has_column_content=bool(
            deferred or swap_wires or measurement_wires or has_readonly_display
        ),
    )
    apply_measurements(qc, measurement_wires, controls, anti_controls)


def is_readonly_display_token(token: str) -> bool:
    return (
        token in DISPLAY_TOKENS
        or bool(AMPLITUDE_DISPLAY_RE.fullmatch(token))
        or bool(PROBABILITY_DISPLAY_RE.fullmatch(token))
        or bool(DENSITY_DISPLAY_RE.fullmatch(token))
    )


def apply_gate(
    qc: Any,
    wire: int,
    token: str,
    controls: list[int],
    anti_controls: list[int],
    qubits: int,
    basis: BasisTracker,
) -> None:
    if wire >= qubits:
        raise CircuitBuildError("gate references a wire beyond qubits")
    base, angle = split_parametric(token)
    upper = base.upper()
    qft = parse_qft_token(base)
    if qft is not None:
        qft_controls, qft_anti_controls = qft_external_controls(
            wire, qft.span, controls, anti_controls
        )
        apply_qft(qc, wire, qft, qubits, qft_controls, qft_anti_controls)
        track_qft(basis, qft_controls, qft_anti_controls, wire, qft.span)
        return
    if (controls or anti_controls) and base in {"|0>", "|1>"}:
        apply_controlled_write(
            qc,
            basis,
            wire,
            target=0 if base == "|0>" else 1,
            controls=controls,
            anti_controls=anti_controls,
        )
        return
    if controls or anti_controls:
        apply_controlled_gate(qc, wire, base, angle, controls, anti_controls)
        track_controlled_gate(basis, controls, anti_controls, wire, base)
        return

    if upper == "H":
        qc.h(wire)
        basis[wire] = None
    elif upper == "X":
        qc.x(wire)
        track_basis_flip(basis, wire)
    elif upper == "Y":
        qc.y(wire)
        track_basis_flip(basis, wire)
    elif upper == "Z":
        qc.z(wire)
    elif is_sqrt_x_token(base):
        qc.sx(wire)
        basis[wire] = None
    elif upper == "S":
        qc.s(wire)
    elif upper in {"S†", "SDG"}:
        qc.sdg(wire)
    elif upper == "T":
        qc.t(wire)
    elif upper in {"T†", "TDG"}:
        qc.tdg(wire)
    elif upper == "P":
        qc.p(parse_angle(angle), wire)
    elif upper == "RX":
        qc.rx(parse_angle(angle), wire)
        basis[wire] = None
    elif upper == "RY":
        qc.ry(parse_angle(angle), wire)
        basis[wire] = None
    elif upper == "RZ":
        qc.rz(parse_angle(angle), wire)
    elif base == "|0>":
        apply_write0(qc, basis, wire)
    elif base == "|1>":
        apply_write1(qc, basis, wire)
    else:
        raise CircuitBuildError(f"unsupported gate token: {token}")


def apply_measurements(
    qc: Any,
    measurement_wires: list[int],
    controls: list[int],
    anti_controls: list[int],
) -> None:
    if not measurement_wires:
        return
    # qni/WebGPU does not model controlled Measurement; column controls are ignored.
    del controls, anti_controls
    for wire in measurement_wires:
        qc.measure(wire, wire)


def apply_swap(
    qc: Any,
    swap_wires: list[int],
    controls: list[int],
    anti_controls: list[int],
    basis: BasisTracker,
) -> None:
    if len(swap_wires) != 2:
        return
    first, second = swap_wires
    if controls or anti_controls:
        apply_controlled_swap(qc, first, second, controls, anti_controls)
        track_controlled_swap(basis, controls, anti_controls, first, second)
        return
    qc.swap(first, second)
    basis[first], basis[second] = basis[second], basis[first]


def apply_controlled_swap(
    qc: Any,
    first: int,
    second: int,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    apply_controlled_x(qc, second, controls + [first], anti_controls)
    apply_controlled_x(qc, first, controls + [second], anti_controls)
    apply_controlled_x(qc, second, controls + [first], anti_controls)


def apply_qft_swaps(
    qc: Any,
    wire: int,
    span: int,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    # Bit-reversal swap network swap(wire+i, wire+span-1-i) for the first half
    # of the span. Any column controls ride along so a controlled-QFT gates the
    # swaps too. The QFT's overall basis effect is set by track_qft (called from
    # apply_gate after apply_qft), so the swaps need no per-step basis update here.
    for i in range(span // 2):
        first = wire + i
        second = wire + span - 1 - i
        if controls or anti_controls:
            apply_controlled_swap(qc, first, second, controls, anti_controls)
        else:
            qc.swap(first, second)


def track_controlled_swap(
    basis: BasisTracker,
    controls: list[int],
    anti_controls: list[int],
    first: int,
    second: int,
) -> None:
    control_values = [basis[control] for control in controls]
    anti_control_values = [basis[anti_control] for anti_control in anti_controls]
    if any(value == 0 for value in control_values) or any(
        value == 1 for value in anti_control_values
    ):
        return
    if any(value is None for value in control_values + anti_control_values):
        if basis[first] is not None and basis[first] == basis[second]:
            return
        basis[first] = None
        basis[second] = None
        return
    basis[first], basis[second] = basis[second], basis[first]


def apply_control_only_z(
    qc: Any,
    controls: list[int],
    anti_controls: list[int],
    basis: BasisTracker,
    *,
    has_column_content: bool,
) -> None:
    # qni/WebGPU drops AntiControl bits for control-only columns.
    del anti_controls
    if has_column_content or len(controls) < 2:
        return
    target = controls[0]
    remaining_controls = controls[1:]
    apply_controlled_gate(qc, target, "Z", None, remaining_controls, [])
    track_controlled_gate(basis, remaining_controls, [], target, "Z")


def parse_qft_token(token: str) -> QftSpec | None:
    if token.startswith("QFT†"):
        suffix = token.removeprefix("QFT†")
        dagger = True
    elif token.startswith("QFT"):
        suffix = token.removeprefix("QFT")
        dagger = False
    else:
        return None
    if not suffix.isdecimal():
        raise CircuitBuildError(f"unsupported gate token: {token}")
    span = int(suffix)
    if span < 1:
        raise CircuitBuildError(f"unsupported gate token: {token}")
    return QftSpec(span=span, dagger=dagger)


def qft_external_controls(
    wire: int,
    span: int,
    controls: list[int],
    anti_controls: list[int],
) -> tuple[list[int], list[int]]:
    def outside_qft_span(control: int) -> bool:
        return control < wire or control >= wire + span

    return (
        [control for control in controls if outside_qft_span(control)],
        [anti_control for anti_control in anti_controls if outside_qft_span(anti_control)],
    )


def apply_qft(
    qc: Any,
    wire: int,
    spec: QftSpec,
    qubits: int,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    if wire + spec.span > qubits:
        raise CircuitBuildError("QFT span references a wire beyond qubits")
    # The bare H/phase ladder realizes the DFT on bit-reversed input (F·BR);
    # the bit-reversal swaps go before the ladder (forward) / after the inverse
    # ladder (dagger) so the net transform is the true symmetric DFT, matching
    # Quirk and the GPU path's linearize_qft.
    if spec.dagger:
        for offset in reversed(range(spec.span)):
            for distance in reversed(range(1, spec.span - offset)):
                apply_qft_phase(
                    qc,
                    -math.pi / (1 << distance),
                    control=wire + offset + distance,
                    target=wire + offset,
                    controls=controls,
                    anti_controls=anti_controls,
                )
            apply_qft_h(qc, wire + offset, controls, anti_controls)
        apply_qft_swaps(qc, wire, spec.span, controls, anti_controls)
        return
    apply_qft_swaps(qc, wire, spec.span, controls, anti_controls)
    for offset in range(spec.span):
        apply_qft_h(qc, wire + offset, controls, anti_controls)
        for distance in range(1, spec.span - offset):
            apply_qft_phase(
                qc,
                math.pi / (1 << distance),
                control=wire + offset + distance,
                target=wire + offset,
                controls=controls,
                anti_controls=anti_controls,
            )


def apply_qft_h(
    qc: Any, wire: int, controls: list[int], anti_controls: list[int]
) -> None:
    if controls or anti_controls:
        apply_controlled_gate(qc, wire, "H", None, controls, anti_controls)
        return
    qc.h(wire)


def apply_qft_phase(
    qc: Any,
    phase: float,
    *,
    control: int,
    target: int,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    if controls or anti_controls:
        apply_controlled_phase(qc, phase, target, controls + [control], anti_controls)
        return
    qc.cp(phase, control, target)


def mark_unknown(basis: BasisTracker, wire: int, span: int) -> None:
    for offset in range(span):
        basis[wire + offset] = None


def track_qft(
    basis: BasisTracker,
    controls: list[int],
    anti_controls: list[int],
    wire: int,
    span: int,
) -> None:
    control_values = [basis[control] for control in controls]
    anti_control_values = [basis[anti_control] for anti_control in anti_controls]
    if any(value == 0 for value in control_values) or any(
        value == 1 for value in anti_control_values
    ):
        return
    mark_unknown(basis, wire, span)


def apply_controlled_gate(
    qc: Any,
    wire: int,
    base: str,
    angle: str | None,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    upper = base.upper()
    if upper == "X":
        apply_controlled_x(qc, wire, controls, anti_controls)
        return
    gate = controlled_gate_for_base(qc, base, angle)
    for anti_control in anti_controls:
        qc.x(anti_control)
    control_wires = controls + anti_controls
    append_controlled_gate(qc, gate, control_wires, [wire])
    for anti_control in reversed(anti_controls):
        qc.x(anti_control)


def apply_controlled_phase(
    qc: Any,
    phase: float,
    wire: int,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    gate = controlled_phase_gate(qc, phase)
    for anti_control in anti_controls:
        qc.x(anti_control)
    control_wires = controls + anti_controls
    append_controlled_gate(qc, gate, control_wires, [wire])
    for anti_control in reversed(anti_controls):
        qc.x(anti_control)


def controlled_phase_gate(qc: Any, phase: float) -> Any:
    if not is_real_qiskit_circuit(qc):
        return ("P", phase)
    from qiskit.circuit.library import PhaseGate  # type: ignore[import-not-found]

    return PhaseGate(phase)


def controlled_gate_for_base(qc: Any, base: str, angle: str | None) -> Any:
    upper = base.upper()
    token = controlled_gate_token(base)
    if not is_real_qiskit_circuit(qc):
        return (token, angle)
    from qiskit.circuit.library import (  # type: ignore[import-not-found]
        HGate,
        PhaseGate,
        RXGate,
        RYGate,
        RZGate,
        SGate,
        SdgGate,
        SXGate,
        TGate,
        TdgGate,
        YGate,
        ZGate,
    )

    if token == "H":
        return HGate()
    if token == "Y":
        return YGate()
    if token == "Z":
        return ZGate()
    if token == "SX":
        return SXGate()
    if upper == "S":
        return SGate()
    if upper in {"S†", "SDG"}:
        return SdgGate()
    if upper == "T":
        return TGate()
    if upper in {"T†", "TDG"}:
        return TdgGate()
    if upper == "P":
        return PhaseGate(parse_angle(angle))
    if upper == "RX":
        return RXGate(parse_angle(angle))
    if upper == "RY":
        return RYGate(parse_angle(angle))
    if upper == "RZ":
        return RZGate(parse_angle(angle))
    raise CircuitBuildError(f"controlled gate token is not supported: {base}")


def controlled_gate_token(base: str) -> str:
    upper = base.upper()
    if is_sqrt_x_token(base):
        return "SX"
    if upper in {"H", "Y", "Z", "S", "T", "P", "RX", "RY", "RZ"}:
        return upper
    if upper in {"S†", "SDG"}:
        return "S†"
    if upper in {"T†", "TDG"}:
        return "T†"
    raise CircuitBuildError(f"controlled gate token is not supported: {base}")


def is_sqrt_x_token(base: str) -> bool:
    return base in {"√X", "X^½"}


def is_diagonal_gate_token(base: str) -> bool:
    if base.upper() == "X":
        return False
    return controlled_gate_token(base) in {"Z", "S", "S†", "T", "T†", "P", "RZ"}


def is_real_qiskit_circuit(qc: Any) -> bool:
    return qc.__class__.__module__.startswith("qiskit")


def append_controlled_gate(
    qc: Any, gate: Any, control_wires: list[int], target_wires: list[int]
) -> None:
    if is_real_qiskit_circuit(qc):
        qc.append(gate.control(len(control_wires)), control_wires + target_wires)
    else:
        qc.append(("controlled", gate, list(control_wires), list(target_wires)))


def apply_controlled_x(
    qc: Any, wire: int, controls: list[int], anti_controls: list[int]
) -> None:
    for anti_control in anti_controls:
        qc.x(anti_control)
    qc.mcx(controls + anti_controls, wire)
    for anti_control in reversed(anti_controls):
        qc.x(anti_control)


def track_basis_flip(basis: BasisTracker, wire: int) -> None:
    if basis[wire] is not None:
        basis[wire] ^= 1


def track_controlled_gate(
    basis: BasisTracker,
    controls: list[int],
    anti_controls: list[int],
    wire: int,
    base: str,
) -> None:
    control_values = [basis[control] for control in controls]
    anti_control_values = [basis[anti_control] for anti_control in anti_controls]
    if any(value == 0 for value in control_values) or any(
        value == 1 for value in anti_control_values
    ):
        return
    if is_diagonal_gate_token(base):
        return
    if any(value is None for value in control_values + anti_control_values):
        basis[wire] = None
        return
    upper = base.upper()
    if upper in {"X", "Y"}:
        track_basis_flip(basis, wire)
    elif upper in {"H", "RX", "RY"} or is_sqrt_x_token(base):
        basis[wire] = None


def apply_controlled_write(
    qc: Any,
    basis: BasisTracker,
    wire: int,
    *,
    target: int,
    controls: list[int],
    anti_controls: list[int],
) -> None:
    control_values = [basis[control] for control in controls]
    anti_control_values = [basis[anti_control] for anti_control in anti_controls]
    if any(value == 0 for value in control_values) or any(
        value == 1 for value in anti_control_values
    ):
        return
    value = basis[wire]
    if value is None:
        raise CircuitBuildError(
            f"|{target}> requires a deterministic basis state in the dev Qiskit runner"
        )
    if value == target:
        return
    apply_controlled_x(qc, wire, controls, anti_controls)
    track_controlled_gate(basis, controls, anti_controls, wire, "X")


def apply_write0(qc: Any, basis: BasisTracker, wire: int) -> None:
    apply_write(qc, basis, wire, target=0)


def apply_write1(qc: Any, basis: BasisTracker, wire: int) -> None:
    apply_write(qc, basis, wire, target=1)


def apply_write(qc: Any, basis: BasisTracker, wire: int, *, target: int) -> None:
    value = basis[wire]
    if value is None:
        raise CircuitBuildError(
            f"|{target}> requires a deterministic basis state in the dev Qiskit runner"
        )
    if value != target:
        qc.x(wire)
        basis[wire] = target
