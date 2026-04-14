# Tech Stack

このプロジェクトは **Rust を中心にしたモノレポ** で、用途ごとに 3 つのアプリで構成されています。

- **Web PoC**: ブラウザ上で動く量子回路 UI
- **TUI PoC**: ターミナル上で動く確認用 UI
- **MCP サーバ**: 回路編集・状態ベクトル取得用のローカルサーバ

## 全体像

- リポジトリ構成: **monorepo**
- 主言語: **Rust** / **JavaScript (Node.js)**
- Rust toolchain: **stable** (`rust-toolchain.toml`)
- Web 向けビルド: **Rust → WebAssembly**
- Node パッケージ管理: **pnpm**
- CI: **GitHub Actions**

## アプリごとの技術スタック

### 1. `apps/egui-web` — Web フロントエンド

ブラウザで動く WebGPU PoC です。

- 言語: **Rust 2021**
- UI: **egui / eframe**
- GPU API: **wgpu**（**WebGPU** バックエンド）
- 配布形態: **WebAssembly (`wasm32-unknown-unknown`)**
- ビルド/開発サーバ: **Trunk**
- ブラウザ側の起動補助: **ES Modules の `bootstrap.js`**
- テスト: **Playwright + Chromium**

要するに、一般的な React/Vue ベースの Web アプリではなく、**Rust で書いた egui アプリを Wasm としてブラウザ上で動かし、描画と状態計算に WebGPU を使う構成**です。

主な依存関係:

- `eframe = 0.33.3`
- `wgpu = 27.0.1`
- `wasm-bindgen`
- `web-sys`
- `@playwright/test = 1.57.0`

### 2. `apps/tui` — ターミナル UI

Web 版とは別に、ローカルで素早く確認できる TUI PoC です。

- 言語: **Rust 2021**
- UI: **ratatui**
- 端末入出力: **crossterm**
- テスト: **cargo test** + **insta** + **ratatui-testlib**

主な依存関係:

- `ratatui = 0.26`
- `crossterm = 0.27`
- `insta = 1`

### 3. `apps/mcp-qni` — MCP サーバ

Codex / Claude などの MCP クライアントから量子回路を操作するためのサーバです。

- 実行環境: **Node.js**
- モジュール形式: **ESM** (`"type": "module"`)
- SDK: **@modelcontextprotocol/sdk**
- 通信方式: **stdio**
- パッケージ管理: **pnpm**
- 品質チェック: **ESLint / Prettier / Node test runner**

主な依存関係:

- `@modelcontextprotocol/sdk`（package.json では `^1.0.0`、lockfile では 1.x 系を解決）
- `eslint`（9.x）
- `prettier`（3.x）

## 開発・検証ツール

### Rust 側

- `cargo fmt`
- `cargo clippy -- -D warnings`
- `cargo test`
- `cargo audit`
- `cargo deny`

### Web 側

- `trunk serve`
- `pnpm exec playwright test`
- WebAssembly target: `wasm32-unknown-unknown`

### CI

GitHub Actions では以下を使って検証します。

- **Node.js 20**
- **pnpm 9**
- **Rust stable**
- **trunk**
- **Playwright Chromium**

補足:
- ローカル Linux や Playwright MCP の運用では **xvfb-run** を使うことがある
- WebGPU 実行時は Chromium のフラグ経由で **SwiftShader** を使う構成を取る

## 実際のビルド/実行単位

- Web UI を動かす: `apps/egui-web`
- TUI を動かす: `apps/tui`
- MCP サーバを動かす: `apps/mcp-qni`
- 全体チェック: `./scripts/check-all.sh`

## ひとことで言うと

このプロジェクトは、**Rust を中心に、Web は egui + wgpu + WebAssembly + Trunk、TUI は ratatui、外部連携は Node.js 製 MCP サーバで構成されたモノレポ**です。
