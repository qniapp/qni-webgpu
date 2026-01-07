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

## WebGPU PoC の確認手順（ローカル）

### 1) インストールして起動
```
cd apps/web
pnpm install
pnpm dev -- --host 127.0.0.1 --port 4173
```

### 2) WebGPU 対応ブラウザで開く
- `http://127.0.0.1:4173/` を開く
- WebGPU が無効なら有効化する:
  - Chrome: `chrome://flags` で "WebGPU" を有効化

### 3) 表示される内容
- 1本の量子ビット線と H ゲート箱
- 状態ベクトル表示: `[(0.7071067811865475+0i), (0.7071067811865475+0i)]`

補足:
- `?gate=Y` のように指定すると、`X/H/Y/Z/S/T` の表示を切り替えられる（例: `http://127.0.0.1:4173/?gate=Y`）

### ゲートアイコンの更新
SVG を更新した場合は PNG を再生成する。

```
cd apps/web
pnpm icons
```

## Rust (egui) WebGPU PoC（ローカル）

```
cd apps/egui-web
trunk serve --host 127.0.0.1 --port 4174 --no-open
```

ブラウザで `http://127.0.0.1:4174/` を開く。

詳細は `docs/egui-web.md` を参照。

## Playwright での確認（任意）
WebGPU の描画を読み戻して検証する。

### Playwright を準備
```
cd apps/web
pnpm install
pnpm exec playwright install chromium
```

### Lint（TypeScript）
```
cd apps/web
pnpm lint
```

### Lint + テストまとめて実行
```
cd apps/web
pnpm check
```

### xvfb でテスト実行（Linux）
```
cd apps/web
xvfb-run -a pnpm exec playwright test
```

## GitHub Actions での CI

GitHub Actions では WebGPU を headless で動かす必要があるため、
`xvfb-run` と SwiftShader (software WebGPU) を使って Playwright を実行する。
`apps/web/playwright.config.ts` で必要な Chrome フラグは設定済み。

ワークフロー例: `.github/workflows/ci.yml`

出力される画像:
- `/tmp/qni-webgpu-canvas.png`
- `/tmp/qni-webgpu-webgpu.png`

## まとめてチェック（トップディレクトリ）
```
./scripts/check-all.sh
```

内部で以下を実行する:
- `apps/web` で `pnpm check`
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
