# QNI WebGPU ABCI Open OnDemand アプリ

このディレクトリは、ABCI の Open OnDemand Sandbox Apps に配置する Batch Connect アプリ定義。
`qni-webgpu` の Web UI と Qiskit GPU バックエンドを Singularity / Apptainer イメージとして GPU ノード上で起動する。

## 配置

```bash
mkdir -p "$HOME/ondemand/dev/qni-webgpu"
rsync -a --delete deploy/abci_ood/ "$HOME/ondemand/dev/qni-webgpu/"
```

Open OnDemand の "My Sandbox Apps" から QNI WebGPU を選び、以下を入力する。

- `Singularity / Apptainer Image Path`: `$HOME/qni-webgpu.sif` のような絶対パス
- `ABCI Resource Type`: 利用する GPU 資源種別
- `ABCI Group`: ABCI グループ名
- `Basic Authentication`: 認証なし、ユーザー名 / パスワード生成、既存 `.htpasswd` のいずれか

## 実行時の制約

- 本番ランナーは `qiskit-gpu` のみ。
- `mock` と `qiskit-cpu-dev` は Open OnDemand 実行時に許可しない。
- Web UI は `/node/<host>/<port>/` 配下から静的ファイルを読み、API は同じ接続元の `/run` へ送る。
