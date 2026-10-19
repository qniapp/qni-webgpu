---
name: verify-kani
description: controls.rs、qubit_bit.rs、simulation_plan/linearize.rs の算術パラメータや制御除去、simulation_plan/arithmetic_field.rs、arithmetic.wgsl を変更するとき、制御保存と算術の添字写像を Kani で再検証する。
---

# 制御条件と算術の添字写像を検証する

## いつ使うか

次の変更では、リポジトリのルートでこの手順を実行する。

- `apps/web/src/gates/controls.rs` と `apps/web/src/qubit_bit.rs`
- `apps/web/src/simulation_plan/linearize.rs` の算術パラメータや `register_external_controls`
- `apps/web/src/simulation_plan/arithmetic_field.rs` の `ArithmeticField`
- `apps/web/src/gpu/shaders/arithmetic.wgsl`
- ローカル量子ビット数の上限、検証クレート、検証スクリプト

## 性質と影響

1. `restricted_outside` は範囲内の制御だけを消し、範囲外の `mask` と `value` を保存する。壊れると、本来と異なる基底状態に作用する。
2. 算術の逆引き添字は範囲内かつ単射、したがって全単射である。壊れると、振幅が重複・欠落する。
3. 対象範囲外のビットは不変で、制御成立時の範囲内は `field - amount` の `2^span` を法とする値、制御不成立時は元の添字である。壊れると、無関係な量子ビットを書き換えたり、加減算の方向を誤ったりする。
4. `amount = 1` と `u32::MAX` の写像を同じ制御で合成すると恒等写像になる。壊れると、加減算で元の状態に戻せない。
5. 実コードの対象マスクは指定幅に一致して量子ビット数内に収まり、制御は範囲外だけに残る。壊れると、制御付き算術が置換であることを保証できない。

## コマンドと判定

```bash
bash apps/web/scripts/verify-kani.sh
```

`pnpm -C apps/web run verify:kani` も同じ入口である。
未導入なら `cargo install --locked kani-verifier --version 0.68.0 && cargo kani setup` を実行する。
Kani はホスト上で専用の nightly を使う。wasm32 を指定しない。

- 実ソースの各ハーネスに `VERIFICATION:- SUCCESSFUL` が必要で、集計は `Complete - 5 successfully verified harnesses, 0 failures, 5 total.` でなければならない。
- 故障版はハーネスごとに一時コピーで作り、対象だけを実行する。`VERIFICATION:- FAILED` と `Complete - 0 successfully verified harnesses, 1 failures, 1 total.` が必要である。
- スクリプトが各故障版の `Failed Checks:` 行を完全一致で確認する。既存の制御保存では `Failed Checks: restricted_outside clears inside bits and preserves outside mask/value bits` を維持する。
- 正常版の成功と、すべての故障版の期待した失敗を満たした終了コード 0 だけを成功とする。故障版の成功やコンパイル失敗は検証手順の異常である。
- 記録を残す場合は `KANI_LOG_DIR` に保存先を指定する。一時コピーは自動で削除される。

### ハーネスと故障注入の対応

| ハーネス名（モジュール名を除く） | 故障注入 |
| --- | --- |
| `restricted_outside_removes_only_inside_controls` | 実 Rust の `!span_mask` を `span_mask` にする |
| `arithmetic_permutation` | 転記の減算後のマスクを除去する |
| `arithmetic_locality` | 転記の対象マスクをシフトしない |
| `arithmetic_inverse` | 実 Rust の減算量を `u32::MAX - 1` にする |
| `arithmetic_plan_preconditions` | 実 Rust の対象マスクを `(1u32 << span) - 2` にする |
| `arithmetic_plan_preconditions` | 実 Rust の制御除去で対象範囲をシフトしない |

## 失敗したとき

1. バージョン・コンパイル・ソース取り込みのエラーは反例ではない。Kani 0.68.0 と相対パスを確認する。
2. シェーダの一致確認に失敗したら、WGSL とハーネス内の転記を比較する。古い期待値だけを新しいコードに合わせて成功させない。
3. 実ソースのアサーション失敗なら、`cargo kani --manifest-path apps/web/verification/kani/Cargo.toml -Z concrete-playback --concrete-playback=print` で反例を調べる。`inplace` は使わない。
4. 制御保存では `mask`、`value`、`span_mask` を比較する。範囲内の残存や範囲外の変化は実装の不具合である。
5. 算術では `n`、`wire`、`span`、制御、`amount`、添字を確認する。範囲外への参照・衝突、対象外ビットの変化、加減算で戻らない例をコードの言葉で記録する。
6. 実 Rust の取り込み、WGSL との対応、折り返す `u32` 演算、以下の前提を確認する。転記の誤りなら検証側の不具合である。成功のためだけに前提を追加したり上限を下げたりしない。

## 上限・前提と実コードへの対応

独立クレート `apps/web/verification/kani` が実際の Rust ソースを取り込む。
制御保存は任意の `u32` 3 個を対象とし、前提は `value & !mask == 0` だけで、全 32 ビットを扱う。

算術は `1 <= n <= 16`、`wire < n`、`1 <= span <= n-wire`、添字は `0..2^n`、量は `1` または `u32::MAX`。
16 は `LOCAL_MAX_QUBITS` に対応し、外部 GPU の上限 32 とは区別する。
制御は下位 `n` ビット内、`value` は `mask` の部分集合である。
計画の証明では対象内の制御も与えて実コードによる除去を検証し、写像の証明では対象外の制御を使う。
幅は `linearize.rs` の `GateSpan::clamped_for` による切り詰め後を前提とし、切り詰め関数そのものや入力生成を証明しない。
ループの展開上限による打ち切りはない。

製品の添字写像は WGSL だけに置き、CPU に追加しない。
ハーネスだけにある転記は WGSL の `u32` 減算を `wrapping_sub` で表す。
スクリプトがシェーダの対応箇所を固定して変更を検知するが、意味同値の自動証明ではないため転記のレビューが必要である。
GPU 実行・バッファ配置・振幅の数値精度・回路全体の順序・QFT・wasm32 専用コードは対象外であり、既存のテストを置き換えない。

## 意図的な仕様変更

先に新しい性質と利用側への影響を明文化し、レビューを受ける。
その後、実コード、`apps/web/verification/kani/lib.rs`、故障注入、シェーダの一致確認、このスキルと [説明文書](../../../docs/formal-verification.md) を合わせて更新する。
実 Rust の取り込みを維持し、各ハーネスはアサーション 1 つにする。
各故障注入の置換箇所は一意で、正常版は成功し、故障版は対応するアサーションで失敗することを再確認する。

## スキル自体の動作確認

未保存の変更を確認し、他の変更を上書きしないようにする。
実際の作業ツリーの `controls.rs` で `let keep = !span_mask;` だけを一時的に `let keep = span_mask;` に変える。
上のコマンドが実ソースの段階で非 0 終了し、`Failed Checks: restricted_outside clears inside bits and preserves outside mask/value bits` を含むことを確認する。
この試行では、同じ制御除去を使う算術ハーネスも失敗してよい。
一時変更だけを戻して再実行し、終了コード 0 と `git diff -- apps/web/src/gates/controls.rs` に差分がないことを確認する。
実ソースを壊して検出し、戻して成功するまでを一組として実行する。他の人の変更を消さない。
