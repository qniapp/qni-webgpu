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

```
cd apps/tui
cargo fmt
cargo clippy -- -D warnings
cargo test
```

詳細は `docs/rust.md` を参照。

依存関係チェック:

```
cd apps/tui
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

## Rust (egui) WebGPU PoC（ローカル）

まずサーバを起動する:

```
cd apps/egui-web
trunk serve --address 127.0.0.1 --port 4174
```

通常のブラウザ起動では WebGPU adapter を取れず、白画面やエラーメッセージになることがある。
Linux / Wayland での今後のローカル動作確認は、別ターミナルから **フラグ付き Google Chrome** を helper script で開く運用を正本とする:

```
./scripts/open-egui-web.sh
```

この helper は `google-chrome-stable` を優先し、見つからない場合のみ Chromium 系へ fallback する。

直接開く場合の URL は `http://127.0.0.1:4174/`。

詳細は `docs/egui-web.md` を参照。

## Playwright での確認（任意）
WebGPU の描画を読み戻して検証する。

### Playwright を準備
```
cd apps/egui-web
pnpm install
pnpm exec playwright install chromium
```

### xvfb でテスト実行（Linux）
```
cd apps/egui-web
xvfb-run -a -s "-screen 0 1920x1080x24" pnpm exec playwright test
```

## GitHub Actions での CI

GitHub Actions では `./scripts/check-all.sh` 経由で staged rollout の Web gate を通す。
`apps/egui-web` では `pnpm run test:preflight` → `pnpm run test:bdd` → `pnpm run test:pw-legacy` の順で実行し、
legacy 側の `test:pw-legacy` が `apps/egui-web/playwright.config.cjs` の WebGPU フラグ付き Playwright を呼ぶ。
ローカルでは `playwright.config.cjs` も `google-chrome-stable` を優先し、
未インストール時のみ Playwright 同梱 Chromium へ fallback する。Linux 環境では `xvfb-run` を併用できる。

ワークフロー例: `.github/workflows/ci.yml`

出力される画像:
- `/tmp/qni-egui-webgpu-initial.png`
- `/tmp/qni-egui-webgpu-after.png`

## まとめてチェック（トップディレクトリ）
```
./scripts/check-all.sh
```

内部で以下を実行する:
- `apps/egui-web` で `pnpm run test:preflight`（Chrome 優先解決の browser preflight）
- `apps/egui-web` で `pnpm run test:bdd`（Cucumber BDD）
- `apps/egui-web` で `pnpm run test:pw-legacy`（legacy Playwright）
- `apps/mcp-qni` で `pnpm check`
- ルートで `make check`（TUI 向け）

## MCP サーバ（Qni）

回路編集と状態ベクトル取得を行う MCP サーバは `apps/mcp-qni` にある。

```
cd apps/mcp-qni
pnpm install
pnpm start
```

Claude Code でプロジェクトに登録する:

```
claude mcp add --scope project --transport stdio qni -- \
  node /home/yasuhito/Work/qni-webgpu/apps/mcp-qni/src/index.js
```

詳細は `docs/mcp-qni.md` を参照。
