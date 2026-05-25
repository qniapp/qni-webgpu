#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${QNI_SMOKE_IMAGE:-qni-webgpu-abci}"
CONTAINER="${QNI_SMOKE_CONTAINER:-qni-webgpu-smoke-$$}"
HOST="${QNI_SMOKE_HOST:-127.0.0.1}"
PORT="${QNI_SMOKE_PORT:-8000}"
AUTH_USERNAME="${QNI_SMOKE_AUTH_USERNAME:-qni-smoke}"
AUTH_PASSWORD="${QNI_SMOKE_AUTH_PASSWORD:-qni-smoke-password}"
BUILD_IMAGE="${QNI_SMOKE_BUILD:-true}"
RUN_GPU_CHECK="${QNI_SMOKE_RUN_GPU:-true}"
if [[ -z "${QNI_SMOKE_DOCKER_GPU_ARGS+x}" ]]; then
  DOCKER_GPU_ARGS="--gpus all"
else
  DOCKER_GPU_ARGS="${QNI_SMOKE_DOCKER_GPU_ARGS}"
fi

validate_bool() {
  case "$2" in
    true|false) ;;
    *)
      echo "$1 must be true or false" >&2
      exit 1
      ;;
  esac
}

validate_bool QNI_SMOKE_BUILD "${BUILD_IMAGE}"
validate_bool QNI_SMOKE_RUN_GPU "${RUN_GPU_CHECK}"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_command curl
require_command docker
require_command python3

if [[ "${BUILD_IMAGE}" == "true" ]]; then
  docker build -t "${IMAGE}" "${ROOT_DIR}"
fi

cleanup() {
  docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup

# shellcheck disable=SC2206
GPU_ARGS=( ${DOCKER_GPU_ARGS} )
docker run -d --rm \
  --name "${CONTAINER}" \
  "${GPU_ARGS[@]}" \
  -p "${HOST}:${PORT}:8000" \
  -e QNI_REQUIRE_BASIC_AUTH=true \
  -e QNI_AUTH_USERNAME="${AUTH_USERNAME}" \
  -e QNI_AUTH_PASSWORD="${AUTH_PASSWORD}" \
  "${IMAGE}" >/dev/null

health_url="http://${HOST}:${PORT}/health"
run_url="http://${HOST}:${PORT}/run"

for _ in $(seq 1 120); do
  if curl -fsS "${health_url}" >/tmp/qni-smoke-health.json 2>/dev/null; then
    break
  fi
  sleep 1
done
curl -fsS "${health_url}" >/tmp/qni-smoke-health.json

python3 - <<'PY'
import json
from pathlib import Path
body = json.loads(Path('/tmp/qni-smoke-health.json').read_text())
if body.get('defaultRunner') != 'qiskit-gpu' or body.get('runners') != ['qiskit-gpu']:
    raise SystemExit(f'unexpected health payload: {body!r}')
PY

auth_status=$(curl -sS -o /tmp/qni-smoke-noauth.txt -w '%{http_code}' \
  -H 'content-type: application/json' \
  -d '{"qubits":1,"columns":[],"outputs":{"histogram":true}}' \
  "${run_url}")
if [[ "${auth_status}" != "401" ]]; then
  echo "expected unauthenticated /run to return 401, got ${auth_status}" >&2
  exit 1
fi

mock_status=$(curl -sS -u "${AUTH_USERNAME}:${AUTH_PASSWORD}" -o /tmp/qni-smoke-mock.txt -w '%{http_code}' \
  -H 'content-type: application/json' \
  -d '{"runner":"mock","qubits":1,"columns":[],"outputs":{"histogram":true}}' \
  "${run_url}")
if [[ "${mock_status}" != "400" ]]; then
  echo "expected production container to reject mock runner with 400, got ${mock_status}" >&2
  exit 1
fi

if [[ "${RUN_GPU_CHECK}" == "true" ]]; then
  curl -fsS -u "${AUTH_USERNAME}:${AUTH_PASSWORD}" \
    -H 'content-type: application/json' \
    -d '{"qubits":1,"columns":[["H"]],"shots":16,"outputs":{"histogram":true}}' \
    "${run_url}" >/tmp/qni-smoke-run.json
  python3 - <<'PY'
import json
from pathlib import Path
body = json.loads(Path('/tmp/qni-smoke-run.json').read_text())
if body.get('status') != 'completed' or body.get('runner') != 'qiskit-gpu':
    raise SystemExit(f'unexpected run payload: {body!r}')
PY
else
  echo "skipped GPU /run check because QNI_SMOKE_RUN_GPU=false" >&2
fi

echo "qni-webgpu ABCI container smoke test passed"
