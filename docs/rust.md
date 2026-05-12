# Rust 開発チェック

このリポジトリは `rust-toolchain.toml` で stable を指定している。

## フォーマット

```
cd apps/tui
cargo fmt
```

## Lint（Clippy）

```
cd apps/tui
cargo clippy -- -D warnings
```

## テスト

```
cd apps/tui
cargo test
```

## 依存関係 / Snapshot チェック

事前にインストール:

```
cargo install cargo-audit cargo-deny cargo-insta
```

未承認 snapshot 確認:

```
cd apps/tui
cargo insta pending-snapshots
```

脆弱性チェック:

```
cd apps/tui
cargo audit
```

ライセンス/依存ポリシー:

```
cd apps/tui
cargo deny check --config ../../deny.toml
```

## ワンコマンド

```
./scripts/check.sh
```

または:

```
make check
```
