---
name: verify-kani-controls
description: controls.rs、qubit_bit.rs、simulation_plan/linearize.rs の register_external_controls などの呼び出し側を変更するとき、restricted_outside の制御保存を Kani で再検証する。
---

# 制御条件の保存を検証する

## 対象と性質

`apps/web/src/gates/controls.rs` の `ColumnControls::restricted_outside(span_mask)` は範囲内の制御だけを取り除き、範囲外の各ビットの `mask` と `value` を変えない。
これが壊れると、QFT や算術ゲートが誤った基底状態に作用する。
`controls.rs`、依存先の `apps/web/src/qubit_bit.rs`、呼び出し側の `apps/web/src/simulation_plan/linearize.rs` などを変更したら実行する。呼び出し側の変更では前提と利用方法も確認する。この証明だけでは呼び出し側の正しさを保証しない。

## コマンドと判定

リポジトリのルートで実行する。

```bash
bash apps/web/scripts/verify-kani-controls.sh
```

未導入なら `cargo install --locked kani-verifier --version 0.68.0 && cargo kani setup` を実行する。
Kani はホスト上で専用の nightly を使う。wasm32 を指定しない。

- 実ソースで `VERIFICATION:- SUCCESSFUL` と `Complete - 1 successfully verified harnesses, 0 failures` が必要である。
- 一時コピーの `let keep = !span_mask;` を `let keep = span_mask;` に変えた故障版では、`VERIFICATION:- FAILED` と `Complete - 0 successfully verified harnesses, 1 failures` が必要である。さらに `Failed Checks: restricted_outside clears inside bits and preserves outside mask/value bits` の行が完全一致することも確認する。
- 両方を満たした終了コード 0 だけを成功とする。故障版の成功やコンパイル失敗も検証手順の異常として扱う。
- 記録を残す場合は `KANI_CONTROLS_LOG_DIR` に保存先を指定する。

## 失敗したとき

1. 導入・コンパイル・ソース取り込みのエラーは反例ではない。Kani のバージョンと相対パスを確認する。
2. 実ソースのアサーション失敗なら、`cargo kani --manifest-path apps/web/verification/kani-controls/Cargo.toml -Z concrete-playback --concrete-playback=print` で反例の入力を調べる。`inplace` は使わず、ソースへのテスト追記を避ける。
3. `mask`、`value`、`span_mask` と結果を比較する。範囲内にビットが残るか、範囲外のビットが変われば実装の不具合である。
4. 前提 `value & !mask == 0`、実ソースの取り込み、仕様の 4 条件が正しいか確認する。取り込み先や前提が誤っていれば検証側を修正する。成功させるためだけに前提を追加したり対象ビットを減らしたりしない。

故障版では、たとえば範囲外にある制御が消える。これは本来の制御条件で選ばれる基底状態を変える反例である。

## 意図的な仕様変更

先に新しい性質と利用側への影響を明文化し、レビューを受ける。
その後、`apps/web/verification/kani-controls/lib.rs` の `RestrictedOutsideSpec`、このスキル、[説明文書](../../../docs/formal-verification.md) を合わせて更新する。
実装のロジックを検証用にコピーせず、実ソースの取り込みを保つ。各検証ハーネスのアサーションは 1 つだけにする。
新しい仕様でも故障を検出できるよう故障注入箇所を見直し、正常版の成功と故障版の失敗を再確認する。

## スキル自体の動作確認

未保存の変更を退避し、`controls.rs` の `let keep = !span_mask;` だけを一時的に `let keep = span_mask;` に変える。
上のコマンドが実ソースの段階で非 0 終了することを確認する。
一時変更だけを戻して再実行し、終了コード 0 と `git diff -- apps/web/src/gates/controls.rs` に意図しない差分がないことを確認する。他の人の変更を消さない。

## 範囲と限界

`kani::any()` による任意の `u32` 3 個と `kani::assume(value & !mask == 0)` を使い、全 32 ビットを対象とする。入力範囲の制限やループの打ち切りはない。
独立クレートには外部依存がなく、実ソースの非公開フィールドを子モジュールから直接生成する。
通常の `#[cfg(test)]` 内のテストは Kani の証明対象ではない。
WGSL、GPU、他の `ColumnControls` メソッド、呼び出し側の範囲生成、wasm32 専用コードは対象外であり、既存のテストを置き換えない。
