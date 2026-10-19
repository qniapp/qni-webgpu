---
name: verify-kani
description: controls.rs、qubit_bit.rs、simulation_plan/linearize.rs、simulation_plan/arithmetic_field.rs、gpu/arithmetic_index.rs、build_support/arithmetic_codegen.rs、arithmetic.wgsl のひな型を変更するとき、制御保存・計画の前提条件・算術添字写像を Kani と生成テストで再検証する。
---

# 制御条件・算術パラメータ・添字写像を検証する

## いつ使うか

次の変更では、リポジトリのルートでこの手順を実行する。

- `apps/web/src/gates/controls.rs` と `apps/web/src/qubit_bit.rs`
- `apps/web/src/simulation_plan/linearize.rs` の算術パラメータや `register_external_controls`
- `apps/web/src/simulation_plan/arithmetic_field.rs` の `ArithmeticField`
- `apps/web/src/gpu/arithmetic_index.rs`、`apps/web/build_support/arithmetic_codegen.rs` とシェーダのひな型
- ローカル量子ビット数の上限、検証クレート、検証スクリプト

## 性質と影響

1. `restricted_outside` は範囲内の制御だけを消し、範囲外の `mask` と `value` を保存する。壊れると、本来と異なる基底状態に作用する。
2. `ArithmeticField` の先頭位置は `wire` に一致し、対象マスクは指定幅に正確に一致して量子ビット数内に収まる。除去後の制御は範囲外だけに残り、範囲外の値は保存され、`value` は `mask` の部分集合である。加算量はインクリメントで `1`、デクリメントで `u32::MAX` になる。壊れると、誤ったパラメータをシェーダに渡してしまう。
3. `gpu/arithmetic_index.rs` の実 Rust を取り込み、添字写像の全単射・範囲外ビット保存・正負の量による逆写像・方向・制御不成立時の恒等性を証明する。方向は「参照元に量を加えると出力先」と定義し、符号反転を検出する。壊れると、誤った基底状態から振幅を読み込む。

## コマンドと判定

```bash
bash apps/web/scripts/verify-kani.sh
```

`pnpm -C apps/web run verify:kani` も同じ入口である。
未導入なら `cargo install --locked kani-verifier --version 0.68.0 && cargo kani setup` を実行する。
Kani はホスト上で専用の nightly を使う。wasm32 を指定しない。

- 実ソースの各ハーネスに `VERIFICATION:- SUCCESSFUL` が必要で、集計は `Complete - 7 successfully verified harnesses, 0 failures, 7 total.` でなければならない。
- 8 件の故障版はそれぞれ一時コピーで作り、対象ハーネスだけを実行する。`VERIFICATION:- FAILED` と `Complete - 0 successfully verified harnesses, 1 failures, 1 total.` が必要である。
- スクリプトが各故障版の `Failed Checks:` 行を完全一致で確認する。制御保存では `Failed Checks: restricted_outside clears inside bits and preserves outside mask/value bits`、計画の前提条件では `Failed Checks: arithmetic plan has exact mask fit disjoint controls and correct amount` が必要である。
- 正常版の成功と、すべての故障版の期待した失敗を満たした終了コード 0 だけを成功とする。故障版の成功やコンパイル失敗は検証手順の異常である。
- 記録を残す場合は `KANI_LOG_DIR` に保存先を指定する。一時コピーは自動で削除される。

### ハーネスと故障注入の対応

| ハーネス名（モジュール名を除く） | 故障注入 |
| --- | --- |
| `restricted_outside_removes_only_inside_controls` | 実 Rust の `!span_mask` を `span_mask` にする（#118 の内外反転） |
| `arithmetic_plan_preconditions` | 実 Rust の減算量を `u32::MAX - 1` にする |
| `arithmetic_plan_preconditions` | 実 Rust の対象マスクを `(1u32 << span) - 2` にする |
| `arithmetic_plan_preconditions` | 実 Rust の制御除去で対象範囲をシフトしない |
| `arithmetic_permutation` | 対象フィールドをゼロに潰す |
| `arithmetic_outside_bits` | 範囲外ビットを消す |
| `arithmetic_inverse` | 加減算量を常に `1` にする |
| `arithmetic_direction` | 減算を加算に反転する |

## 失敗したとき

1. バージョン・コンパイル・ソース取り込みのエラーは反例ではない。Kani 0.68.0 と相対パスを確認する。
2. ローカル上限の確認に失敗したら、`LOCAL_MAX_QUBITS` と証明範囲を照合する。
3. 実ソースのアサーション失敗なら、`cargo kani --manifest-path apps/web/verification/kani/Cargo.toml -Z concrete-playback --concrete-playback=print` で反例を調べる。`inplace` は使わない。
4. 制御保存では `mask`、`value`、`span_mask` を比較する。範囲内の残存や範囲外の変化は実装の不具合である。
5. 計画の前提条件では `n`、`wire`、`span`、制御、加減算量を確認する。マスクの幅・位置、制御の残存や消失、加減算量の誤りを記録する。
6. 添字写像では `i`、`shift`、`mask`、`amount`、制御を確認する。添字写像の 4 種類の `Failed Checks:` 行は[説明文書](../../../docs/formal-verification.md#同じ-rust-ソースからの生成と証明)を参照する。
7. 実 Rust の取り込みと以下の前提を確認する。成功のためだけに前提を追加したり上限を下げたりしない。

## 上限・前提と実コードへの対応

独立クレート `apps/web/verification/kani` が実際の Rust ソースを取り込む。
`linearize.rs` の算術分岐も、証明対象の `ArithmeticField` を使う。
制御保存は任意の `u32` 3 個を対象とし、前提は `value & !mask == 0` だけで、全 32 ビットを扱う。

計画の前提条件は `1 <= n <= 16`、`wire < n`、`1 <= span <= n-wire` を仮定する。
16 は `LOCAL_MAX_QUBITS` に対応し、スクリプトが定数の一致を確認する。外部 GPU の上限 32 は対象外である。
入力の制御は下位 `n` ビット内、`value` は `mask` の部分集合である。
対象範囲内の制御も入力に与え、実コードによる除去を検証する。インクリメントかどうかの真偽値は任意である。
幅は `linearize.rs` の `GateSpan::clamped_for` による切り詰め後を前提とし、切り詰め関数そのものや入力生成を証明しない。
各ハーネスはアサーション 1 つで、ループの展開上限による打ち切りはない。

## 対象外と GPU 比較テスト

添字写像は幅 `1..=16`、先頭 `0..=32-width`、任意の `u32` の添字と対象範囲外の制御で証明する。量は `1` または `u32::MAX` とし、桁あふれも扱う。
WGSL への変換と実行の等価性は形式検証の対象外である。変換器の構文拒否テストと GPU 比較を併用する。
既存の `apps/web/test-gpu/arithmetic.test.ts` は、入力振幅を順方向に配置した参照値と、逆引きで振幅を集める実際のシェーダ出力を比較する。
幅 `1, 2, 3, 4, 16`、加減算量 `1, -1` を試す。
幅 16 は制御なし、それ以外は制御あり・なしを扱い、制御ありでは対象範囲を 1 ビットずらして正負の制御を組み合わせる。
列挙した条件での比較であり、すべての入力についての証明ではない。
`pnpm -C apps/web run test:gpu` は実 GPU ハードウェアが必要で、CI では実行しない。

添字計算は Rust に一度だけ記述してビルド時に WGSL を生成し、本番の CPU では呼ばない。GPU テストも生成物を使い、手書きシェーダへの代替経路は設けない。
GPU 実行・バッファ配置・振幅の数値精度・回路全体の順序・QFT・他の `ColumnControls` メソッド・wasm32 専用コードも Kani の対象外であり、既存のテストを置き換えない。

## 意図的な仕様変更

先に新しい性質と利用側への影響を明文化し、レビューを受ける。
その後、実コード、`apps/web/verification/kani/lib.rs`、故障注入、このスキルと [説明文書](../../../docs/formal-verification.md) を合わせて更新する。
実 Rust の取り込みを維持し、各ハーネスはアサーション 1 つにする。
各故障注入の置換箇所は一意で、正常版は成功し、故障版は対応するアサーションで失敗することを再確認する。

## スキル自体の動作確認

未保存の変更を確認し、他の変更を上書きしないようにする。
実際の作業ツリーの `controls.rs` で `let keep = !span_mask;` だけを一時的に `let keep = span_mask;` に変える。
上のコマンドが実ソースの段階で非 0 終了し、`Failed Checks: restricted_outside clears inside bits and preserves outside mask/value bits` を含むことを確認する。
この試行では、同じ制御除去を使う算術ハーネスも失敗してよい。
一時変更だけを戻して再実行し、終了コード 0 と `git diff -- apps/web/src/gates/controls.rs` に差分がないことを確認する。
実ソースを壊して検出し、戻して成功するまでを一組として実行する。他の人の変更を消さない。
