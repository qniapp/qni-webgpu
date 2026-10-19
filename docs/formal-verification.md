# 制御条件と算術ゲートの形式検証

Kani 0.68.0 で、実際の Rust コードの制御除去と算術パラメータの生成を検証する。
既存の単体テストでは列挙しにくい、対象範囲と制御の全組み合わせを扱うために Kani を使う。
WGSL の添字写像の置換・局所性・逆写像は形式検証していない。

## 実行方法

リポジトリのルートで、初回のみ Kani 0.68.0 を準備する。
Kani はホスト上で専用の nightly を使い、プロジェクトの stable や wasm32 向けのビルドとは独立して動作する。

```bash
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
```

ローカルでも CI でも、検証は次の 1 コマンドで行う。

```bash
bash apps/web/scripts/verify-kani.sh
```

`pnpm -C apps/web run verify:kani` でも同じ検証を行う。
実ソースの 2 ハーネスの成功と、4 件の故障版の期待した失敗を確認した場合だけ終了コードが 0 になる。
故障版は一時コピーだけを書き換え、対象のハーネスだけを実行する。終了時に一時ファイルを削除する。
`KANI_LOG_DIR` を指定すると、正常版と故障版のログをそのディレクトリに保存できる。

## CI での実行

[CI ワークフロー](../.github/workflows/ci.yml) の `web-kani` ジョブが、master への push と、master または feat/embed-restart-button 宛てのプルリクエストで同じスクリプトを実行する。
実ソースの 2 件の証明と 4 件の故障検出を省略せず、失敗・中断した場合は集約ジョブ `web` も成功しない。
GPU は不要で、他の Web 検証と並列に動作する。ジョブの制限時間は導入処理を含めて 15 分である。

[公式の CI 導入手順](https://model-checking.github.io/kani/install-github-ci.html) に従い、`model-checking/kani-github-action@v1.1` を使う。
Action に `v0.68.0` タグはなく、v1.0 以降は Action と Kani の版が独立しているため、リリースタグ `v1.1` と `kani-version: '0.68.0'` を指定する。
`command: bash` と `args: apps/web/scripts/verify-kani.sh` で検証対象をスクリプトに一元化し、スクリプト側でも Kani の版を確認する。

## 検証する性質と実コードとの対応

依存ライブラリのない独立クレート `apps/web/verification/kani` が、実際の `controls.rs`、`qubit_bit.rs` と `simulation_plan/arithmetic_field.rs` を取り込む。
`simulation_plan/linearize.rs` の算術分岐は、この `ArithmeticField` を使う。

- **制御の保存**: `ColumnControls::restricted_outside` は範囲内の制御だけを消し、範囲外の `mask` と `value` を変えない。
  壊れると、QFT や算術ゲートが本来と異なる基底状態に作用する。
- **計画の前提条件**: `ArithmeticField` の先頭位置は `wire` に一致し、対象マスクは指定幅に正確に一致して量子ビット数内に収まる。
  除去後の制御は対象範囲と重ならず、範囲外の制御は保存され、`value` は `mask` の部分集合である。
  加算量はインクリメントで `1`、デクリメントで `u32::MAX` になる。
  壊れると、シェーダに誤った対象範囲・制御・加減算量を渡してしまう。

## 上限と前提

制御保存の証明は任意の `u32` の `mask`、`value`、`span_mask` を扱い、前提は `value & !mask == 0` だけである。
連続範囲に限定せず、全 32 ビットを対象とする。

計画の前提条件の証明は次の範囲を扱う。

- 量子ビット数 `n` は `1..=16`。これは `LOCAL_MAX_QUBITS` に対応する。
  スクリプトが定数の一致を確認する。外部 GPU の上限 32 は対象に含めない。
- 対象の先頭 `wire` は `0..n`、幅 `span` は `1..=n-wire`。
  `linearize.rs` が行う `GateSpan::clamped_for` による切り詰め後の値を前提とする。切り詰め関数自体の証明ではない。
- 入力の制御ビットは下位 `n` ビット内にあり、`value` は `mask` の部分集合。
  対象範囲内の制御も入力に含め、実コードによる除去を検証する。
- インクリメントかどうかを表す真偽値は任意である。

各ハーネスはアサーションを 1 つだけ持つ。ループの展開上限による打ち切りはない。
計画全体や呼び出し側の有効な入力の生成まで証明したものではない。

## 故障検出

次の 4 件を一時コピーに注入する。

| ハーネス | 故障注入 |
| --- | --- |
| `restricted_outside_removes_only_inside_controls` | `!span_mask` を `span_mask` にする（#118 の内外反転） |
| `arithmetic_plan_preconditions` | 減算量を `u32::MAX - 1` にする |
| `arithmetic_plan_preconditions` | 対象マスクを `(1u32 << span) - 2` にする |
| `arithmetic_plan_preconditions` | 制御除去で対象範囲のシフトを省略する |

スクリプトは置換箇所が一意であること、Kani の非 0 終了、対応する `Failed Checks:` 行の完全一致、`VERIFICATION:- FAILED` と失敗数を確認する。
コンパイルエラーだけでは故障検出の成功にならない。

## 対象外と GPU 比較テスト

WGSL の添字写像の置換・局所性・逆写像は形式検証の対象外であり、既存の `apps/web/test-gpu/arithmetic.test.ts` の GPU 比較テストで確認する。
このテストは入力振幅を順方向の写像で配置した参照値を生成し、逆引きで振幅を集める実際のシェーダの出力と比較する。
幅は `1, 2, 3, 4, 16`、加減算量は `1, -1` を扱う。
幅 16 は制御なし、それ以外は制御あり・なしを試し、制御ありでは対象範囲を 1 ビットずらして正負の制御を組み合わせる。
これは列挙した条件での比較であり、すべての入力についての証明ではない。

```bash
pnpm -C apps/web run test:gpu
```

`test:gpu` は実 GPU ハードウェアが必要で、CI では実行しない。
振幅の数値精度、GPU 実行・バッファ配置・並列処理、WGSL コンパイラ、回路全体の順序、QFT、他の `ColumnControls` メソッド、wasm32 専用コードも Kani の対象外である。
既存の Rust・GPU テストを置き換えない。
変更時の判断と再検証の手順は、プロジェクト内の [検証スキル](../.agents/skills/verify-kani/SKILL.md) を参照する。
