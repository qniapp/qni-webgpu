from __future__ import annotations

import ast
import math
import re
from typing import Any

CONTROL_TOKENS = {"•", "●", "control", "Control"}
ANTI_CONTROL_TOKENS = {"○", "anti", "Anti"}
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
    if expr is None or not expr.strip():
        return default
    normalized = expr.strip().replace("π", "pi")
    tree = ast.parse(normalized, mode="eval")
    return float(_eval_angle_node(tree.body))


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
            return left / right
    raise CircuitBuildError("unsupported angle expression")


def split_parametric(token: str) -> tuple[str, str | None]:
    match = re.fullmatch(r"([^()]+)(?:\((.*)\))?", token)
    if not match:
        return token, None
    base, angle = match.groups()
    return base, angle.replace("_", "/") if angle is not None else None


BasisTracker = list[int | None]


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
    deferred: list[tuple[int, str]] = []
    for wire, raw in enumerate(column):
        token = token_text(raw)
        if token is None:
            continue
        if token in CONTROL_TOKENS:
            controls.append(wire)
            continue
        if token in ANTI_CONTROL_TOKENS:
            raise CircuitBuildError("anti-control is not supported by the dev Qiskit runner yet")
        if is_readonly_display_token(token):
            continue
        deferred.append((wire, token))
    for wire, token in deferred:
        apply_gate(qc, wire, token, controls, qubits, basis)


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
    qubits: int,
    basis: BasisTracker,
) -> None:
    if wire >= qubits:
        raise CircuitBuildError("gate references a wire beyond qubits")
    base, angle = split_parametric(token)
    upper = base.upper()
    if controls and upper != "X":
        raise CircuitBuildError(
            "controlled non-X gates are not supported by the dev Qiskit runner yet"
        )
    if controls:
        qc.mcx(controls, wire)
        track_controlled_x(basis, controls, wire)
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
    elif base == "√X":
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
    elif upper.startswith("QFT"):
        raise CircuitBuildError("QFT tokens are not supported by the dev Qiskit runner yet")
    else:
        raise CircuitBuildError(f"unsupported gate token: {token}")


def track_basis_flip(basis: BasisTracker, wire: int) -> None:
    if basis[wire] is not None:
        basis[wire] ^= 1


def track_controlled_x(basis: BasisTracker, controls: list[int], wire: int) -> None:
    control_values = [basis[control] for control in controls]
    if any(value == 0 for value in control_values):
        return
    if any(value is None for value in control_values):
        basis[wire] = None
        return
    track_basis_flip(basis, wire)


def apply_write0(qc: Any, basis: BasisTracker, wire: int) -> None:
    value = basis[wire]
    if value is None:
        raise CircuitBuildError(
            "|0> requires a deterministic basis state in the dev Qiskit runner"
        )
    if value == 1:
        qc.x(wire)
        basis[wire] = 0
