# qni Qiskit backend

Local development backend for the egui-web external GPU execution path.

## Runners

- `mock`: fixed histogram response. No quantum simulation; useful for UI/API smoke tests.
- `qiskit-cpu-dev`: explicit local Qiskit CPU runner for checking the Qiskit circuit path on machines without CUDA. This is not a WebGPU fallback and must not be used as the production execution path.
- `qiskit-gpu`: Qiskit Aer statevector runner with `device="GPU"` and `cuStateVec_enable=True`. No CPU fallback.

All runners return bounded histogram output only. The API rejects full statevector / full probability requests. The egui-web state-vector panel, when refreshed for <=16-qubit GPU-mode runs, is recomputed locally in WebGPU after a successful run rather than transferred from this backend.

## Run

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner mock
```

CPU dev runner (requires a Python environment with `qiskit` and `qiskit-aer`):

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner qiskit-cpu-dev
```

GPU runner:

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner qiskit-gpu
```

## API

Health:

```bash
curl http://127.0.0.1:4184/health
```

Run:

```bash
curl -s http://127.0.0.1:4184/run \
  -H 'content-type: application/json' \
  -d '{"runner":"mock","qubits":2,"columns":[["H",1]],"shots":128,"outputs":{"histogram":true}}'
```

Response shape:

```json
{"status":"completed","runner":"mock","qubits":2,"shots":128,"histogram":{"00":128},"truncated":false}
```

## Test

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m unittest discover apps/qiskit-backend/tests
```
