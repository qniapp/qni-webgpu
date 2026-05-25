from __future__ import annotations

import argparse
import json
import os
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from .circuit import CircuitBuildError
from .contract import ContractError, parse_run_request
from .runners import RunnerUnavailable, select_runner


class QiskitBackendHandler(BaseHTTPRequestHandler):
    server_version = "qni-qiskit-backend/0.1"

    def do_OPTIONS(self) -> None:  # noqa: N802 - stdlib hook
        self.send_response(HTTPStatus.NO_CONTENT.value)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type")
        self.end_headers()

    def do_GET(self) -> None:  # noqa: N802 - stdlib hook
        if self.path == "/health":
            self.write_json(
                HTTPStatus.OK,
                {
                    "status": "ok",
                    "defaultRunner": self.default_runner,
                    "runners": ["mock", "qiskit-cpu-dev", "qiskit-gpu"],
                },
            )
            return
        self.write_json(HTTPStatus.NOT_FOUND, {"error": "not_found"})

    def do_POST(self) -> None:  # noqa: N802 - stdlib hook
        if self.path != "/run":
            self.write_json(HTTPStatus.NOT_FOUND, {"error": "not_found"})
            return
        try:
            payload = self.read_json_body()
            request = parse_run_request(payload, default_runner=self.default_runner)
            result = select_runner(request.runner).run(request)
        except (CircuitBuildError, ContractError) as exc:
            self.write_json(HTTPStatus.BAD_REQUEST, {"error": "bad_request", "message": str(exc)})
            return
        except RunnerUnavailable as exc:
            self.write_json(
                HTTPStatus.SERVICE_UNAVAILABLE,
                {"error": "runner_unavailable", "message": str(exc)},
            )
            return
        except Exception as exc:  # pragma: no cover - defensive server boundary
            self.write_json(
                HTTPStatus.INTERNAL_SERVER_ERROR,
                {"error": "internal_error", "message": str(exc)},
            )
            return
        self.write_json(HTTPStatus.OK, result)

    @property
    def default_runner(self) -> str:
        return getattr(self.server, "default_runner", "mock")

    def read_json_body(self) -> dict[str, Any]:
        length = int(self.headers.get("content-length", "0"))
        raw = self.rfile.read(length)
        payload = json.loads(raw.decode("utf-8"))
        if not isinstance(payload, dict):
            raise ContractError("request body must be a JSON object")
        return payload

    def write_json(self, status: HTTPStatus, payload: dict[str, Any]) -> None:
        data = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        self.send_response(status.value)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("access-control-allow-origin", "*")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002 - stdlib signature
        if os.environ.get("QNI_QISKIT_BACKEND_LOG"):
            super().log_message(format, *args)


class QiskitBackendServer(ThreadingHTTPServer):
    default_runner: str


def make_server(host: str, port: int, default_runner: str) -> QiskitBackendServer:
    server = QiskitBackendServer((host, port), QiskitBackendHandler)
    server.default_runner = default_runner
    return server


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="QNI local Qiskit backend")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", default=4184, type=int)
    parser.add_argument(
        "--runner",
        default=os.environ.get("QNI_QISKIT_RUNNER", "mock"),
        choices=["mock", "qiskit-cpu-dev", "qiskit-gpu"],
        help="default runner for requests that omit runner",
    )
    args = parser.parse_args(argv)
    server = make_server(args.host, args.port, args.runner)
    print(f"qni qiskit backend listening on http://{args.host}:{args.port} runner={args.runner}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
    return 0
