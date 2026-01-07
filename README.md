# qni-webgpu

## TUI PoC（ローカル）

```
cd apps/tui
cargo run
```

ゲートを切り替える場合:

```
cd apps/tui
cargo run -- --gate=Y
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

```
cd apps/egui-web
trunk serve --address 127.0.0.1 --port 4174
```

ブラウザで `http://127.0.0.1:4174/` を開く。

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
xvfb-run -a pnpm exec playwright test
```

## GitHub Actions での CI

GitHub Actions では WebGPU を headless で動かす必要があるため、
`xvfb-run` と SwiftShader (software WebGPU) を使って Playwright を実行する。
`apps/egui-web/playwright.config.cjs` で必要な Chrome フラグは設定済み。

ワークフロー例: `.github/workflows/ci.yml`

出力される画像:
- `/tmp/qni-egui-webgpu-initial.png`
- `/tmp/qni-egui-webgpu-after.png`

## まとめてチェック（トップディレクトリ）
```
./scripts/check-all.sh
```

内部で以下を実行する:
- `apps/egui-web` で Playwright
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
