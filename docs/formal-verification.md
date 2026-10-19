# 制御条件と算術ゲートの形式検証

Kani 0.68.0 で、実際の Rust コードの制御除去、算術パラメータの生成と算術添字写像を検証する。
既存の単体テストでは列挙しにくい、対象範囲と制御の全組み合わせを扱うために Kani を使う。
添字計算は Rust に一度だけ記述し、ビルド時に同じソースから WGSL を生成する。

## 実行方法

リポジトリのルートで、初回のみ Kani 0.68.0 を準備する。
Kani はホスト上で専用の nightly を使い、プロジェクトの stable や wasm32 向けのビルドとは独立して動作する。

```bash
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
```

Kani の準備に時間がかかるため、当面は CI に追加しない。検証は次の 1 コマンドで行う。

```bash
bash apps/web/scripts/verify-kani.sh
```

`pnpm -C apps/web run verify:kani` でも同じ検証を行う。
実ソースの 7 ハーネスの成功と、8 件の故障版の期待した失敗を確認した場合だけ終了コードが 0 になる。
故障版は一時コピーだけを書き換え、対象のハーネスだけを実行する。終了時に一時ファイルを削除する。
`KANI_LOG_DIR` を指定すると、正常版と故障版のログをそのディレクトリに保存できる。

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

次の 8 件を一時コピーに注入する。

| ハーネス | 故障注入 |
| --- | --- |
| `restricted_outside_removes_only_inside_controls` | `!span_mask` を `span_mask` にする（#118 の内外反転） |
| `arithmetic_plan_preconditions` | 減算量を `u32::MAX - 1` にする |
| `arithmetic_plan_preconditions` | 対象マスクを `(1u32 << span) - 2` にする |
| `arithmetic_plan_preconditions` | 制御除去で対象範囲のシフトを省略する |
| `arithmetic_permutation` | 対象フィールドをゼロに潰す |
| `arithmetic_outside_bits` | 再挿入時に範囲外ビットを消す |
| `arithmetic_inverse` | 加減算量を常に `1` にする |
| `arithmetic_direction` | `wrapping_sub(amount)` を `wrapping_add(amount)` にする |

スクリプトは置換箇所が一意であること、Kani の非 0 終了、対応する `Failed Checks:` 行の完全一致、`VERIFICATION:- FAILED` と失敗数を確認する。
コンパイルエラーだけでは故障検出の成功にならない。

## 同じ Rust ソースからの生成と証明

`src/gpu/arithmetic_index.rs` の `source_index` だけが算術添字計算を持つ。
本番の CPU からは呼ばず、`build.rs` が `build_support/arithmetic_codegen.rs` を使い、`syn` で構文を解析して WGSL 関数へ変換する。
変換対象は `u32` の引数・不変な局所変数、括弧・`u32` 接尾辞付き整数リテラル、ビット演算、等値・不一致の比較、シフト、早期リターン、`wrapping_sub` に限定する。
未対応構文はビルドエラーにし、引数と局所変数を `arg0` / `local0` などの名前に付け直す。
`wrapping_sub` は WGSL の符号なし減算、Rust の `!` は WGSL の `~` に対応する。
シェーダのひな型にはバッファ宣言・境界確認・関数呼び出しだけを残す。
完成した `OUT_DIR/arithmetic.wgsl` を本番と GPU テストの両方が読み込む。生成物は追跡しない。

独立した Kani クレートがこの Rust ファイル自体を取り込み、次を各 1 アサーションで証明する。

- **全単射**: 異なる任意の `u32` の添字は異なる参照元へ写る。有限集合上の単射なので全単射となる。
- **範囲外ビットの保存**: 対象フィールドの外は変えない。
- **逆写像**: 正負の量を順に適用すると元に戻る。量は `1` と `u32::MAX` のどちらも記号的に選び、両方の適用順を含める。
- **方向**: 制御成立時、参照元フィールドに量を加えた値が対象幅を法として出力先フィールドに一致する。
  インクリメントは直前の値を参照する。全単射と逆写像だけでは、方向反転の誤りを検出できない。
- **制御不成立**: 添字全体が変わらない。

幅は `1..=16`、先頭位置は `0..=32-width`、添字は任意の `u32`、量は `1` または `u32::MAX` とする。
制御は対象範囲と重ならず、`value` は `mask` の部分集合とする。端での桁あふれや制御不成立も除外しない。
シフトはすべて 32 未満である。計画側の証明と合わせ、ローカル状態数内に添字が収まる。

追加した故障版は次の行で失敗することをスクリプトが完全一致で確認する。

```text
Failed Checks: arithmetic source lookup is injective
Failed Checks: arithmetic preserves outside bits
Failed Checks: arithmetic opposite amounts are inverse
Failed Checks: arithmetic source plus amount equals destination field
```

Kani が証明するのは Rust の性質であり、変換器の正しさや WGSL との等価性ではない。
変換器と演算対応は信頼境界に残る。ネイティブの構文拒否テストと実 GPU の比較で補う。

## 対象外と GPU 比較テスト

既存の `apps/web/test-gpu/arithmetic.test.ts` の GPU 比較テストは、生成した本番用シェーダを使う。
このテストは入力振幅を順方向の写像で配置した参照値を生成し、逆引きで振幅を集める実際のシェーダの出力と比較する。
幅は `1, 2, 3, 4, 16`、加減算量は `1, -1` を扱う。
幅 16 は制御なし、それ以外は制御あり・なしを試し、制御ありでは対象範囲を 1 ビットずらして正負の制御を組み合わせる。
これは列挙した条件での比較であり、すべての入力についての証明ではない。

```bash
pnpm -C apps/web run test:gpu
```

`test:gpu` は実 GPU ハードウェアが必要で、CI では実行しない。
`QNI_GPU_ADAPTER` に利用可能な Vulkan アダプタ名を指定できる。ソフトウェアアダプタは拒否する。Node.js 22.12 以上を使う。
振幅の数値精度、GPU 実行・バッファ配置・並列処理、WGSL コンパイラ、回路全体の順序、QFT、他の `ColumnControls` メソッド、wasm32 専用コードも Kani の対象外である。
既存の Rust・GPU テストを置き換えない。
変更時の判断と再検証の手順は、プロジェクト内の [検証スキル](../.agents/skills/verify-kani/SKILL.md) を参照する。

## 実 GPU での速度比較

算術シェーダ専用の計測を次のように実行する。リポジトリのルートから、リポジトリ外の結果用ディレクトリへ旧版を保存する。`RESULTS_DIR` にその絶対パスを指定しておく。
旧版は比較用の外部ファイルにだけ残し、本番の代替経路には使わない。

```bash
export RESULTS_DIR="${RESULTS_DIR:?リポジトリ外の保存先を指定してください}"
mkdir -p "$RESULTS_DIR"
git show df2834a36839e958bc2102296bb0f9795a50c84f:apps/web/src/gpu/shaders/arithmetic.wgsl \
  > "$RESULTS_DIR/arithmetic-before.wgsl"
cd apps/web
export QNI_GPU_ADAPTER='実際の Vulkan アダプタ名'
node -r ts-node/register/transpile-only --test --test-concurrency=1 test-gpu/arithmetic.test.ts
node -r ts-node/register/transpile-only test-gpu/arithmetic.bench.ts \
  --shader="$RESULTS_DIR/arithmetic-before.wgsl" --output="$RESULTS_DIR/before.json"
node -r ts-node/register/transpile-only test-gpu/arithmetic.bench.ts \
  --baseline="$RESULTS_DIR/arithmetic-before.wgsl" --output="$RESULTS_DIR/paired-after.json"
```

`--shader` を省略すると、Cargo のビルド結果から本番用の生成シェーダを特定する。
`--shader=<WGSL ファイル>` で測定対象、`--baseline=<WGSL ファイル>` で比較対象を指定できる。
結果は標準出力と `--output` の JSON に記録する。結果用ファイルはコミットしない。

状態数 `2^12` と `2^16`、幅 `1, 3, 8, 16`、量 `1, -1` を測る。幅 16 は状態数 `2^16` の制御なしだけを扱う。
それ以外は先頭をずらし、対象範囲外の正負の制御を組み合わせる。
事前にパイプライン・バッファ・コマンドを準備し、256 回の往復ディスパッチを一組としてウォームアップ後に 24 標本を測る。
比較時は旧版・生成版の実行順を交互に変える。測定中に割り当て・エンコード・リードバックを行わない。

値はホストの単調増加時計による投入から `queue.onSubmittedWorkDone()` 完了までの一組の遅延であり、投入・完了通知の費用を含む。
GPU のタイムスタンプによる純粋なシェーダ時間ではない。
中央値、中央絶対偏差、比率、標本数、アダプタ情報、実行環境、シェーダの SHA-256 を保存する。
ドライバ版は WebGPU が公開しない場合があり、その場合はアダプタの説明と測定環境側の記録で補う。
中央値の悪化が 5% を超え、かつその差が両者の中央絶対偏差の和を超える場合は再測定し、再現する悪化を受け入れない。
ばらつきに埋もれる差は結論を保留し、速度向上と主張しない。現時点では実 GPU での等価性確認と速度測定は未実施で、結果は未確定である。実 GPU での正しさや速度を主張しない。
