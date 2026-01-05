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
xvfb-run -a pnpm exec playwright test
```

出力される画像:
- `/tmp/qni-webgpu-canvas.png`
- `/tmp/qni-webgpu-webgpu.png`

## MCP サーバ（Qni）

回路編集と状態ベクトル取得を行う MCP サーバは `apps/mcp-qni` にある。

```
cd apps/mcp-qni
pnpm install
pnpm start
```

詳細は `docs/mcp-qni.md` を参照。
