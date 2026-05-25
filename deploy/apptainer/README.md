# QNI WebGPU Singularity / Apptainer イメージ

ABCI 上で Open OnDemand から起動する `qni-webgpu.sif` を作るための定義。

## 作成

```bash
module load singularitypro
singularity build --fakeroot qni-webgpu.sif deploy/apptainer/qni-webgpu.def
```

既定では `https://github.com/qniapp/qni-webgpu/archive/refs/heads/master.tar.gz` を取得してビルドする。
検証用に別の tarball を使う場合は、ビルド時の環境変数で上書きする。

```bash
QNI_WEBGPU_SOURCE_URL=https://example.com/qni-webgpu.tar.gz \
  singularity build --fakeroot qni-webgpu.sif deploy/apptainer/qni-webgpu.def
```

## 実行

```bash
QNI_HTTP_PORT=8000 singularity run --nv qni-webgpu.sif
```

本番ランナーは `qiskit-gpu` のみ。`mock` / `qiskit-cpu-dev` は Open OnDemand 実行では許可しない。
