# Rust のチェック

このリポジトリは `rust-toolchain.toml` で stable を指定している。対象は `apps/web` と、その中の `circuit-library-model`、`external-gpu-model` である。

## まとめて確認する

リポジトリ直下で次を実行する。

```bash
apps/web/scripts/check-rust.sh
```

このスクリプトは次を行う。

- `circuit-library-model` と `external-gpu-model` の `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`
- `apps/web` の `cargo fmt --check`
- `wasm32-unknown-unknown` 向けの `cargo clippy --tests -- -D warnings`
- `wasm32-unknown-unknown` 向けの `cargo test --no-run`
- `apps/web` の `cargo test --lib` と `arithmetic_codegen` テスト
- `cargo insta pending-snapshots`

事前に `rustup target add wasm32-unknown-unknown` と `cargo install cargo-insta` が必要である。

Web のビルド、BDD、Playwright、Qiskit バックエンドまで含めた確認は `./scripts/check-all.sh` を使う。
