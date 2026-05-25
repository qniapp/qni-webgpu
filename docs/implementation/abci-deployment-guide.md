# ABCI 配備手順書

この手順書は、qni-webgpu を ABCI の GPU ノードで動かすための下書きである。
ABCI 実機ではまだ未検証のため、実機確認後に資源種別やモジュール名を更新する。
本番経路では `qiskit-gpu` だけを許可し、CPU 代替実行は使わない。

## 1. イメージを作成する

Docker を使える環境では、まず本番コンテナを作る。

```bash
docker build -t qni-webgpu-abci .
```

ABCI Open OnDemand から使う Singularity / Apptainer イメージは、定義ファイルから作る。
定義ファイルは既定で GitHub の `master` を取得する。別の tarball を使う場合は `QNI_WEBGPU_SOURCE_URL` を指定する。

```bash
module load singularitypro
singularity build --fakeroot "$HOME/qni-webgpu.sif" deploy/apptainer/qni-webgpu.def
```

## 2. Open OnDemand アプリを配置する

```bash
mkdir -p "$HOME/ondemand/dev/qni-webgpu"
rsync -a --delete deploy/abci_ood/ "$HOME/ondemand/dev/qni-webgpu/"
```

Open OnDemand の Sandbox Apps から QNI WebGPU を開き、次を入力する。

- `Singularity / Apptainer Image Path`: `$HOME/qni-webgpu.sif` のような絶対パス。
- `ABCI Resource Type`: 利用する GPU 資源種別。
- `ABCI Group`: ABCI グループ名。
- `Basic Authentication`: 認証なし、ユーザー名 / パスワード生成、既存 `.htpasswd` のいずれか。

## 3. 接続を確認する

ジョブが起動したら、Open OnDemand の接続リンクから Web UI を開く。
通常の URL は次の形になる。

```text
/node/<host>/<port>/
```

ヘルスチェックは同じ接続元の `/health` で確認する。

```bash
curl "https://<ood-host>/node/<host>/<port>/health"
```

返却される `defaultRunner` と `runners` は `qiskit-gpu` だけでなければならない。

## 4. コンテナのスモークテスト

GPU 付き Docker 環境では、次を実行する。

```bash
scripts/smoke-abci-container.sh
```

このスクリプトは次を確認する。

- `/health` が応答し、許可ランナーが `qiskit-gpu` だけであること。
- Basic 認証なしの `/run` が拒否されること。
- 認証付きでも `runner=mock` が拒否されること。
- `runner` 省略時に `qiskit-gpu` で小さな回路が実行できること。

Docker の GPU ランタイムだけを後で確認したい場合は、GPU 実行だけを一時的に飛ばせる。
これは CPU 代替実行ではなく、起動・認証・ランナー制約だけを確認するモードである。
この場合も既定では Docker に `--gpus all` を渡すため、GPU ランタイムの起動確認は残る。

```bash
QNI_SMOKE_RUN_GPU=false scripts/smoke-abci-container.sh
```

GPU ランタイム引数を環境に合わせる場合は、次のように上書きする。
GPU ランタイム自体も使えないローカル環境で構文と認証経路だけを確認する場合は、空文字にする。

```bash
QNI_SMOKE_DOCKER_GPU_ARGS="--gpus all" scripts/smoke-abci-container.sh
QNI_SMOKE_RUN_GPU=false QNI_SMOKE_DOCKER_GPU_ARGS="" scripts/smoke-abci-container.sh
```

## 5. 停止方法

Open OnDemand から停止する場合は、対象セッションを削除する。
Docker スモークテストは終了時にコンテナを自動削除する。
手動で止める場合は、起動時のコンテナ名を指定する。
既定名は `qni-webgpu-smoke-<pid>` で、固定したい場合は `QNI_SMOKE_CONTAINER` を使う。

```bash
QNI_SMOKE_CONTAINER=qni-webgpu-smoke scripts/smoke-abci-container.sh
docker rm -f qni-webgpu-smoke
```

## 6. ログと切り分け

| 症状 | 見る場所 | 確認すること |
| --- | --- | --- |
| Web UI が開かない | Open OnDemand のジョブ標準出力、nginx 標準エラー | ポート、`/node/<host>/<port>/` の接続、静的ファイル配信。 |
| `/health` が失敗する | Qiskit バックエンド標準出力、nginx 標準エラー | backend ポート、起動失敗、Python import エラー。 |
| `/run` が 401 | nginx アクセスログ、認証設定 | `.htpasswd`、ユーザー名、パスワード。 |
| `/run` が `runner is not allowed` | Qiskit バックエンド応答 | 本番で `mock` / `qiskit-cpu-dev` を要求していないか。 |
| GPU 実行が失敗する | Qiskit バックエンド標準出力、CUDA / Qiskit Aer のエラー | `--nv` / `--gpus all`、CUDA ライブラリ、Qiskit Aer GPU ビルド。 |

ログは標準出力 / 標準エラーに集約する。ABCI ジョブでは Open OnDemand のセッションログから確認する。
