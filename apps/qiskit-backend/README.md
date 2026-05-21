# qni Qiskit backend

Local development backend for the web external GPU execution path.

## Runners

- `mock`: fixed histogram response and fixed ground-state Amplitude / Bloch display payloads. No quantum simulation; useful for UI/API smoke tests.
- `qiskit-cpu-dev`: explicit local Qiskit CPU runner for checking the Qiskit circuit path on machines without CUDA. This is not a WebGPU fallback and must not be used as the production execution path.
- `qiskit-gpu`: Qiskit Aer statevector runner with `device="GPU"` and `cuStateVec_enable=True`. No CPU fallback.

All runners return bounded histogram output and optional Amplitude / Bloch display payloads only. The API accepts 1–32 qubits, rejects full statevector / full probability requests, and currently limits exact Amplitude display extraction to <=16 qubits. Bloch display extraction uses per-axis expectation values. The web state-vector panel, when refreshed for <=16-qubit GPU-mode runs, is recomputed locally in WebGPU after a successful run rather than transferred from this backend.

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
  -d '{"runner":"mock","qubits":2,"columns":[["H",1],["Amps1","Bloch"]],"shots":128,"outputs":{"histogram":true,"amplitudes":[{"gate_id":2,"column":1,"span":1,"base_bit":1,"control_mask":0,"control_value":0,"phase_lock_enabled":true}],"bloch":[{"gate_id":3,"column":1,"wire":1}]}}'
```

Response shape:

```json
{"status":"completed","runner":"mock","qubits":2,"shots":128,"histogram":{"00":128},"truncated":false,"amplitudes":[{"gate_id":2,"span":1,"ket":[[1.0,0.0],[0.0,0.0]],"incoherent":[1.0,0.0],"quality":1.0,"phase_lock_index":0}],"bloch":[{"gate_id":3,"vector":[0.0,0.0,1.0]}]}
```

## Test

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m unittest discover apps/qiskit-backend/tests
```
