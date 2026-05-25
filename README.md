# qni-webgpu

## 前提ツール

Web 側のローカル実行では以下が必要:

```
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
```

## TUI PoC（ローカル）

```
cd apps/tui
cargo run
```

## Rust（TUI）チェック

初回のみ:

```
cargo install cargo-audit cargo-deny cargo-insta
```

```
cd apps/tui
cargo fmt
cargo clippy -- -D warnings
cargo test
```

詳細は `docs/rust.md` を参照。

依存関係 / snapshot チェック:

```
cd apps/tui
cargo insta pending-snapshots
cargo audit
cargo deny check --config ../../deny.toml
```

ワンコマンド:

```
./scripts/check.sh
```

または:

```
make check
```

## Web アプリ（Rust / WebGPU、ローカル）

まずサーバを起動する:

```
cd apps/web
trunk serve --address 127.0.0.1 --port 4174 --no-autoreload
```

通常の Chrome で `http://127.0.0.1:4174/` を開く。
リポジトリルートから helper script を使ってもよい:

```
./scripts/open-web.sh
```

この helper は `google-chrome-stable` を優先し、見つからない場合のみ Chromium 系へ fallback する。WebGPU 用の特別な起動フラグは付けない。

詳細は `docs/web.md` を参照。

## Playwright での確認（任意）
WebGPU の描画を読み戻して検証する。

### Playwright を準備
```
cd apps/web
pnpm install
pnpm exec playwright install chromium
```

### xvfb でテスト実行（Linux）
```
cd apps/web
xvfb-run -a -s "-screen 0 1920x1080x24" pnpm exec playwright test
```

## GitHub Actions での CI

GitHub Actions では staged rollout と同じ検証を個別ステップとして実行する。
`apps/web` では本番 Trunk ビルド、`pnpm run test:bdd`、`pnpm run test:pw-legacy` を順に実行し、
legacy 側の `test:pw-legacy` が `apps/web/playwright.config.ts` の Playwright 設定を呼ぶ。
ローカルでは `playwright.config.ts` も `google-chrome-stable` を優先し、
未インストール時のみ Playwright 同梱 Chromium へ fallback する。CI/headless の安定化用に Playwright 側では WebGPU 起動設定を持つ。Linux 環境では `xvfb-run` を併用できる。

ワークフロー例: `.github/workflows/ci.yml`

出力される画像:
- `/tmp/qni-webgpu-initial.png`
- `/tmp/qni-webgpu-after.png`

## まとめてチェック（トップディレクトリ）
```
./scripts/check-all.sh
```

内部で以下を実行する:
- `apps/web` で `trunk build --release --public-url ./`（本番相対パス配信ビルド）
- `apps/web` で `pnpm run test:preflight`（Chrome 優先解決の browser preflight）
- `apps/web` で `pnpm run test:bdd`（Cucumber BDD）
- `apps/web` で `pnpm run test:pw-legacy`（legacy Playwright）
- ルートで `node --test test-node/*.test.cjs`（配備設定 / CI 設定）
- ルートで Qiskit backend の editable install smoke
- ルートで `PYTHONPATH=apps/qiskit-backend/src python3 -m unittest discover apps/qiskit-backend/tests`
- `apps/mcp-qni` で `pnpm check`
- ルートで `make check`（TUI fmt / clippy / test / snapshot / audit / deny）

## Docker（ABCI 本番配備の土台）

本番用コンテナは Web 静的ファイルと Qiskit GPU backend を同一 nginx 配下で起動する。
本番コンテナ内では `qiskit-gpu` だけを許可し、`mock` / `qiskit-cpu-dev` へは逃がさない。
Docker 配備時の Web UI は同一オリジンの `/run` を使い、ローカル開発の `127.0.0.1:4174` だけ `http://127.0.0.1:4184/run` を既定値にする。
静的ファイルは相対パスでビルドし、nginx は Open OnDemand の `/node/<host>/<port>/` 配下でも配信と `/run` 転送を行う。

```
docker build -t qni-webgpu-abci .
docker run --gpus all --rm -p 8000:8000 \
  -e QNI_AUTH_USERNAME=userA \
  -e QNI_AUTH_PASSWORD=passA \
  qni-webgpu-abci
curl http://127.0.0.1:8000/health
```

`.htpasswd` 認証を必須にしたい場合は `QNI_REQUIRE_BASIC_AUTH=true` を追加する。
既存の `.htpasswd` を使う場合は `QNI_AUTH_HTPASSWD_FILE=/path/to/.htpasswd` を指定する。
GPU 付き Docker 環境での確認には `scripts/smoke-abci-container.sh` を使う。

```bash
scripts/smoke-abci-container.sh
```

## ABCI / Open OnDemand

Open OnDemand アプリ定義は `deploy/abci_ood`、Singularity / Apptainer イメージ定義は `deploy/apptainer/qni-webgpu.def` にある。

```
module load singularitypro
singularity build --fakeroot "$HOME/qni-webgpu.sif" deploy/apptainer/qni-webgpu.def
mkdir -p "$HOME/ondemand/dev/qni-webgpu"
rsync -a --delete deploy/abci_ood/ "$HOME/ondemand/dev/qni-webgpu/"
```

Open OnDemand の Sandbox Apps から QNI WebGPU を選び、`$HOME/qni-webgpu.sif` のような絶対パス、ABCI 資源種別、ABCI グループ、Basic 認証設定を入力する。
アプリは `singularity run --nv` で GPU ノード上に起動し、`/node/<host>/<port>/` 配下で Web UI と `/run` API を提供する。
詳しい手順は `docs/implementation/abci-deployment-guide.md`、外部 GPU API の互換方針は `docs/implementation/external-gpu-api-compatibility.md`、qni-gl との差分は `docs/implementation/qni-gl-migration-notes.md` を参照する。

## Qiskit backend（ローカル開発）

外部 GPU 実行パスの API / UI 確認用 backend は `apps/qiskit-backend` にある。
既定の `mock` runner は量子計算をせず固定 histogram と固定 Amplitude / Bloch / Probability / Density Matrix 表示結果を返す。
`qiskit-cpu-dev` は Qiskit 経路確認用の明示的な CPU runner で、WebGPU の CPU fallback ではない。
本番相当の `qiskit-gpu` は `device="GPU"` / `cuStateVec_enable=True` を要求し、CPU fallback しない。
Web UI の `Run GPU` は histogram と表示ブロック単位の Amplitude / Bloch / Probability / Density Matrix 結果だけを要求する。表示ブロック結果がない 16量子ビット以下の GPU-mode run では、成功後に状態ベクトルパネルをローカル WebGPU で1回だけ更新する。backend から全状態ベクトルや全確率分布は返さない。

```
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner mock
PYTHONPATH=apps/qiskit-backend/src python3 -m unittest discover apps/qiskit-backend/tests
```

詳細は `apps/qiskit-backend/README.md` を参照。

## MCP サーバ（Qni）

回路編集と状態ベクトル取得を行う MCP サーバは `apps/mcp-qni` にある。

```
cd apps/mcp-qni
pnpm install
pnpm run build
pnpm start
```

Claude Code でプロジェクトに登録する:

```
claude mcp add --scope project --transport stdio qni -- \
  node /home/yasuhito/Work/qni-webgpu/apps/mcp-qni/dist/src/index.js
```

詳細は `docs/mcp-qni.md` を参照。
