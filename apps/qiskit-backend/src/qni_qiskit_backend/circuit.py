from __future__ import annotations

import ast
import math
import re
from dataclasses import dataclass, field
from typing import Any, Callable

CONTROL_TOKENS = {"•", "●", "control", "Control"}
ANTI_CONTROL_TOKENS = {"◦", "○", "anti", "Anti"}
EMPTY_TOKENS = {None, 1, "1", ""}
AMPLITUDE_DISPLAY_RE = re.compile(r"^Amps(?:[1-9]|1[0-6])$")
PROBABILITY_DISPLAY_RE = re.compile(r"^Probability(?:[1-9]|1[0-6])?$")
DENSITY_DISPLAY_RE = re.compile(r"^Density(?:[1-8])?$")
DISPLAY_TOKENS = {"Bloch"}
POSTSELECTION_TOKENS = {"|0⟩⟨0|", "|1⟩⟨1|", "|+⟩⟨+|", "|-⟩⟨-|"}


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
IFABLE_TOKENS = {"H", "X", "Y", "Z", "√X", "X^½", "S", "S†", "T", "T†"}


@dataclass
class MeasurementFlags:
    sources: dict[str, Any] = field(default_factory=dict)
    writes: int = 0

    def measure(self, qc: Any, wire: int, name: str) -> None:
        # One immutable classical bit per write, not per wire: a later plain
        # measurement/reset or another variable on that wire must not overwrite
        # an earlier variable. flag_sources.rs publishes writes in wire order,
        # after all column unitaries; the bottom-most write therefore wins.
        if is_real_qiskit_circuit(qc):
            from qiskit import ClassicalRegister

            register = ClassicalRegister(1, f"flag_{self.writes}")
            qc.add_register(register)
            bit = register[0]
        else:
            bit = f"flag_{self.writes}"
        self.writes += 1
        qc.measure(wire, bit)
        self.sources[name] = bit


def split_flag_token(token: str) -> tuple[str, str | None, str | None]:
    """Mirror gates/flag.rs; parametric gates cannot carry URL conditions."""
    if token.startswith("Measure>"):
        return "Measure", token.removeprefix("Measure>").strip() or None, None
    if "<" in token:
        base, name = token.split("<", 1)
        if base not in IFABLE_TOKENS:
            raise CircuitBuildError(f"unsupported gate token: {token}")
        return base, None, name.strip() or None
    return token, None, None


@dataclass(frozen=True)
class QftSpec:
    span: int
    dagger: bool


def apply_columns_to_qiskit(qc: Any, columns: list[list[Any]], qubits: int) -> None:
    basis = initial_basis_tracker(qubits)
    flags = MeasurementFlags()
    for column in columns:
        apply_column_to_qiskit(qc, column, qubits, basis, flag_sources=flags)


def initial_basis_tracker(qubits: int) -> BasisTracker:
    return [0 for _ in range(qubits)]


def apply_column_to_qiskit(
    qc: Any,
    column: list[Any],
    qubits: int,
    basis: BasisTracker,
    *,
    save_displays: Callable[[], None] | None = None,
    flag_sources: MeasurementFlags | None = None,
) -> None:
    flags = flag_sources if flag_sources is not None else MeasurementFlags()
    measurement_names: dict[int, str] = {}
    conditions: dict[int, str] = {}
    controls: list[int] = []
    anti_controls: list[int] = []
    swap_wires: list[int] = []
    measurement_wires: list[int] = []
    detector_targets: list[tuple[int, str]] = []
    detect_control_reset_wires: list[int] = []
    parity_targets = column_parity_targets(column, qubits)
    has_readonly_display = False
    deferred: list[tuple[int, str]] = []
    for wire, raw in enumerate(column):
        token = token_text(raw)
        if token is None:
            continue
        token, write_name, condition_name = split_flag_token(token)
        if write_name is not None:
            measurement_names[wire] = write_name
        if condition_name is not None:
            conditions[wire] = condition_name
        if token == "ZDetectControlReset":
            if wire >= qubits:
                raise CircuitBuildError("ZDetectControlReset references a wire beyond qubits")
            detect_control_reset_wires.append(wire)
            controls.append(wire)
            continue
        if token in {"zpar", "xpar"}:
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
        if token in {"XDetector", "YDetector"}:
            if wire >= qubits:
                raise CircuitBuildError("detector references a wire beyond qubits")
            detector_targets.append((wire, token))
            continue
        if token == "Measure":
            measurement_wires.append(wire)
            continue
        if is_readonly_display_token(token):
            has_readonly_display = True
            continue
        deferred.append((wire, token))
    if parity_targets:
        controls.append(parity_targets[0][0])
    if detector_targets and 1 + len(controls) + len(anti_controls) > 8:
        raise CircuitBuildError("detector target plus controls must be <= 8 qubits")
    # A QFT span can cover a control cell, unlike single-wire gates and swaps.
    # It would make the measured wire quantum again: reset would then differ
    # from WebGPU's per-pair Write0. Refuse rather than change that semantics.
    for wire, token in deferred:
        qft = parse_qft_token(split_parametric(token)[0])
        if qft is not None and any(
            wire <= detector < wire + qft.span
            for detector in detect_control_reset_wires
        ):
            raise CircuitBuildError("QFT span overlaps a ZDetectControlReset wire")
    for wire in detect_control_reset_wires:
        qc.measure(wire, wire)
    # Mirror linearize.rs in place: the topmost parity wire reversibly stores
    # the XOR. Its ordinary control gates the entire column in this frame,
    # including measurements/displays; reversing the setup restores the frame.
    # No helper qubit is needed, even for mixed X/Z parity.
    parity_setup: list[tuple[str, int, int | None]] = [
        ("h", wire, None) for wire, token in parity_targets if token == "xpar"
    ]
    if parity_targets:
        anchor = parity_targets[0][0]
        parity_setup.extend(("cx", wire, anchor) for wire, _ in parity_targets[1:])
    for operation, wire, target in parity_setup:
        apply_parity_step(qc, basis, operation, wire, target)
    for wire, token in detector_targets:
        if token == "YDetector":
            qc.sdg(wire)
        qc.h(wire)
    apply_swap(qc, swap_wires, controls, anti_controls, basis)
    operations = group_same_angle_phases(deferred)
    if parity_targets or conditions or any(token in POSTSELECTION_TOKENS for _, token in deferred):
        # WebGPU expands QFT after ordinary targets, including conditional
        # gates and projectors, even when the QFT starts on an earlier wire.
        operations.sort(key=lambda op: parse_qft_token(split_parametric(op[1])[0]) is not None)
    for wire, token, partners in operations:
        if wire not in conditions:
            apply_gate(qc, wire, token, controls + partners, anti_controls, qubits, basis)
            continue
        bit = flags.sources.get(conditions[wire])
        if bit is None:
            # Never-set flags are false, including writes later in this column.
            continue
        branch_basis = basis[:]
        with qc.if_test((bit, 1)):
            apply_gate(qc, wire, token, controls + partners, anti_controls, qubits, branch_basis)
        # The tracker must describe both classical branches, so later writes
        # retain the existing deterministic-basis restriction rather than guess.
        basis[:] = [before if before == after else None
                    for before, after in zip(basis, branch_basis)]
    apply_control_only_z(
        qc,
        controls,
        anti_controls,
        basis,
        has_column_content=bool(
            deferred
            or parity_targets
            or swap_wires
            or measurement_wires
            or detector_targets
            or detect_control_reset_wires
            or has_readonly_display
        ),
    )
    detector_wires = {wire for wire, _ in detector_targets}
    for wire in sorted(measurement_wires + list(detector_wires)):
        if wire in detector_wires:
            apply_detector_measurement(qc, wire, controls, anti_controls)
            if not any(basis[c] == 0 for c in controls) and not any(
                basis[c] == 1 for c in anti_controls
            ):
                basis[wire] = None
        else:
            apply_measurements(qc, [wire], controls, anti_controls)
            if wire in measurement_names:
                # Also retain the ordinary wire bit, exactly as plain Measure.
                # The repeated measurement has no intervening quantum operation.
                flags.measure(qc, wire, measurement_names[wire])
    for wire, token in reversed(detector_targets):
        qc.h(wire)
        if token == "YDetector":
            qc.s(wire)
    if save_displays is not None:
        save_displays()
    for wire in detect_control_reset_wires:
        qc.reset(wire)
        basis[wire] = 0
    for operation, wire, target in reversed(parity_setup):
        apply_parity_step(qc, basis, operation, wire, target)


def column_parity_targets(column: list[Any], qubits: int) -> list[tuple[int, str]]:
    """Drop markers covered by a resizable span, as WebGPU linearize.rs does."""
    spans: list[tuple[int, int]] = []
    parity: list[tuple[int, str]] = []
    for wire, raw in enumerate(column):
        token = token_text(raw)
        if token is None:
            continue
        if token in {"zpar", "xpar"}:
            if wire >= qubits:
                raise CircuitBuildError("parity control references a wire beyond qubits")
            parity.append((wire, token))
        qft = parse_qft_token(split_parametric(token)[0])
        if qft is not None:
            span = qft.span
        elif is_readonly_display_token(token) and token != "Bloch":
            suffix = re.search(r"\d+$", token)
            span = int(suffix.group()) if suffix else 1
        elif re.fullmatch(r"Sample[1-3]", token):
            span = int(token[-1])
        else:
            continue
        spans.append((wire, min(wire + span, qubits)))
    return [(wire, token) for wire, token in parity
            if not any(start <= wire < end for start, end in spans)]


def apply_parity_step(
    qc: Any, basis: BasisTracker, operation: str, wire: int, target: int | None
) -> None:
    if operation == "h":
        qc.h(wire)
        basis[wire] = None
    else:
        assert target is not None
        qc.cx(wire, target)
        track_controlled_gate(basis, [wire], [], target, "X")


def apply_detector_measurement(
    qc: Any, target: int, controls: list[int], anti_controls: list[int]
) -> None:
    if not controls and not anti_controls:
        qc.measure(target, target)
        return
    if not is_real_qiskit_circuit(qc):
        qc.append(("detector_projector", target, list(controls), list(anti_controls)))
        return
    import numpy as np
    from qiskit.quantum_info import Kraus

    # Qarg i occupies bit i: target=1, positive controls=1, anti-controls=0.
    qargs = [target] + controls + anti_controls
    dimension = 1 << len(qargs)
    click_index = (1 << (1 + len(controls))) - 1
    projector = np.zeros((dimension, dimension), dtype=complex)
    projector[click_index, click_index] = 1
    qc.append(Kraus([projector, np.eye(dimension) - projector]), qargs)


def postselection_registers(qc: Any) -> list[Any]:
    return [register for register in getattr(qc, "cregs", ())
            if register.name.startswith("postselect_")]


def apply_postselection(
    qc: Any, target: int, token: str, controls: list[int], anti_controls: list[int]
) -> None:
    if not is_real_qiskit_circuit(qc):
        qc.append(("postselect", target, token, list(controls), list(anti_controls)))
        return
    from qiskit import ClassicalRegister, QuantumRegister

    # Measure only the rejection predicate into an ancilla. Measuring the target
    # itself would destroy coherence between inactive controlled branches.
    # Keeping ancilla=0 implements I-C+C*P, including the sign of the X projector.
    # A single non-TP Kraus matrix in Aer does NOT renormalize. A valid
    # measurement dilation instead normalizes on-device, with no state readback.
    # This samples acceptance: finite-shot misses and WebGPU's 1e-12 cutoff
    # cannot be distinguished here (documented in docs/postselection.md).
    auxiliary = next((r for r in qc.qregs if r.name == "postselection_aux"), None)
    if auxiliary is None:
        auxiliary = QuantumRegister(1, "postselection_aux")
        qc.add_register(auxiliary)
    bit = ClassicalRegister(1, f"postselect_{len(postselection_registers(qc))}")
    qc.add_register(bit)
    x_basis = token in {"|+⟩⟨+|", "|-⟩⟨-|"}
    keep_one = token in {"|1⟩⟨1|", "|-⟩⟨-|"}
    if x_basis:
        qc.h(target)
    for wire in anti_controls:
        qc.x(wire)
    if keep_one:
        qc.x(target)
    qc.mcx(controls + anti_controls + [target], auxiliary[0])
    if keep_one:
        qc.x(target)
    for wire in reversed(anti_controls):
        qc.x(wire)
    qc.measure(auxiliary[0], bit[0])
    qc.reset(auxiliary[0])
    if x_basis:
        qc.h(target)


def group_same_angle_phases(
    deferred: list[tuple[int, str]],
) -> list[tuple[int, str, list[int]]]:
    """Fold same-angle `P` gates of one column into one multi-controlled phase.

    Mirrors the WebGPU `PhaseGroupKey`: the top-most `P` of each angle keeps
    the phase and the other wires join its controls (qni CPHASE / CCPHASE).
    A bare `P` takes the π/2 default, so it groups with `P(π_2)`.
    """
    phase_groups: dict[float, list[int]] = {}
    for wire, token in deferred:
        base, angle = split_parametric(token)
        if base.upper() == "P":
            phase_groups.setdefault(parse_angle(angle), []).append(wire)
    folded: list[tuple[int, str, list[int]]] = []
    for wire, token in deferred:
        base, angle = split_parametric(token)
        if base.upper() != "P":
            folded.append((wire, token, []))
            continue
        target, *partners = phase_groups[parse_angle(angle)]
        if wire == target:
            folded.append((wire, token, partners))
    return folded


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
    if base in POSTSELECTION_TOKENS:
        apply_postselection(qc, wire, base, controls, anti_controls)
        if any(basis[c] == 0 for c in controls) or any(basis[c] == 1 for c in anti_controls):
            return
        if not controls and not anti_controls and base in {"|0⟩⟨0|", "|1⟩⟨1|"}:
            basis[wire] = int(base == "|1⟩⟨1|")
        else:
            basis[wire] = None
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
