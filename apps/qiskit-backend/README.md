# qni Qiskit backend

Local development backend for the web external GPU execution path.

Python と Qiskit のバージョンは `pyproject.toml` で固定する。値の一覧は `docs/tech-stack.md` を参照する。

## Runners

- `mock`: fixed histogram response and fixed ground-state Amplitude / Bloch / Probability / Density Matrix display payloads. No quantum simulation; useful for UI/API smoke tests.
- `qiskit-cpu-dev`: explicit local Qiskit CPU runner for checking the Qiskit circuit path on machines without CUDA. This is not a WebGPU fallback and must not be used as the production execution path.
- `qiskit-gpu`: Qiskit Aer statevector runner with `device="GPU"` and `cuStateVec_enable=True`. No CPU fallback.

Production deployment must start the server with `--runner qiskit-gpu --allowed-runners qiskit-gpu` or equivalent `QNI_QISKIT_RUNNER=qiskit-gpu` / `QNI_QISKIT_ALLOWED_RUNNERS=qiskit-gpu` environment variables. With that setting, request payloads that ask for `mock` or `qiskit-cpu-dev` are rejected instead of falling back to CPU.

All runners return bounded histogram output and optional Amplitude / Bloch / Probability / Density Matrix display payloads only. The API accepts 1–32 qubits, rejects full statevector / full probability requests, and currently limits exact Amplitude display extraction to <=16 qubits. Bloch display extraction uses per-axis expectation values when uncontrolled, and controlled Bloch display requests carry `control_mask` / `control_value` for conditional extraction; target plus controls must stay within 8 saved bits. Probability display extraction uses bounded subsystem probability vectors for the requested display span, and controlled Probability display requests carry `control_mask` / `control_value` for conditional extraction; `span + non-display control count` must stay within 16 saved bits. Density Matrix display extraction uses bounded subsystem density matrices for Quirk-compatible span 1–8 displays, and controlled Density Matrix display requests carry `control_mask` / `control_value` for conditional extraction; `span + non-display control count` must stay within 8 saved bits. The web state-vector panel, when refreshed for <=16-qubit GPU-mode runs, is recomputed locally in WebGPU after a successful run rather than transferred from this backend.

## 対応ゲート

単一量子ビットゲート、制御・反制御、パリティ制御 (`zpar` / `xpar`)、書き込み、Swap、QFT / QFT†、インクリメント・デクリメント (`inc1`〜`inc16` / `dec1`〜`dec16`)、測定、X 軸検出器・Y 軸検出器、Z Detect-Control-Reset、事後選択、測定変数・条件付きゲートに対応する。安全に変換できない制御付き書き込みなどの制限は引き続き適用する。サンプル表示ブロック (`Sample1`〜`Sample8`) は状態を変えず、確率表示と同じ条件付き周辺確率を `probability` で返す。Web 側の共通 GPU シェーダがブロックの ID を種として 1 回抽出するため、バックエンドで試行ごとの標本は生成しない。測定・検出器・条件分岐後の確率は試行平均であり、WebGPU の単一分岐とは異なり得る。

パリティ制御は `xpar` への H と、ほかのパリティ量子ビットから一番上の集約先への CX で集約する。集約先を通常制御と組み合わせ、表示値の保存後に逆変換するため、パリティ制御用の補助量子ビットは不要である。列内の処理順序や重なりの扱いは[パリティ制御の仕様](../../docs/sample-parity.md#外部-gpu-実行-qiskit)、対応範囲は[外部 GPU API 互換方針](../../docs/implementation/external-gpu-api-compatibility.md#ゲート対応)を参照する。

X 軸検出器・Y 軸検出器の制御付き射影測定 `{P, I-P}` は、対象と制御・反制御（パリティの集約先を含む）の合計が 8 量子ビット以内なら密な Kraus 演算子を使う。8 量子ビットを超える場合は、検出条件を補助量子ビットへ多重制御 X で書き込み、リセットして同じ射影測定を実現する。補助量子ビット `postselection_aux` は事後選択と共有し、毎回 `|0⟩` に戻す。Aer は `qubits + 1` 量子ビットを計算し、状態ベクトルのメモリ使用量は補助量子ビットなしの約 2 倍になる。API の容量は入力回路の量子ビット数に適用し、補助量子ビットをヒストグラムや表示の保存対象に含めない。

インクリメント・デクリメントは、範囲内の下位ビットがすべて 1 であることと外部制御・外部反制御を条件とする多重制御 X で実現する。インクリメントは上位ビットから、デクリメントは下位ビットから順に適用し、位相は付けない。基底追跡では、外部制御が確実に不成立の場合を除いて範囲内のワイヤを不定とみなす。Z Detect-Control-Reset の行と範囲が重なる回路と、条件付きの `inc3<a` などは拒否する。詳細は[算術ゲート](../../docs/arith-gates.md#外部-qiskit-実行)を参照する。

## 測定変数・条件付きゲート

`Measure>name` と、条件付きの `H` / `X` / `Y` / `Z` / `X^½` / `S` / `S†` / `T` / `T†` に対応する。
たとえば `X<a` は変数 `a` が 1 のときだけ実行する。変数名の前後の空白は除き、空なら通常ゲートとして扱う。
`P` / `Rx` / `Ry` / `Rz` など、WebGPU の URL 形式で条件を保存できない種類への条件指定は拒否する。

- 各書き込みに専用の古典ビット `flag_<n>` を用意する。同じ量子ビットの再測定や別の変数への書き込みで、以前の変数を上書きしないためである。通常の測定先の回路ビットにも同じ結果を記録する。
- `if_test((clbit, 1))` 内で条件付きゲートを実行する。制御・反制御付きのゲートもこの中で実行する。
- 条件は前の列までの最新の書き込みを参照する。各列ではゲートを先に実行し、その後で測定を上から順に実行するため、同じ列の書き込みは読まず、一番下の書き込みが次の列以降の値になる。未設定ならゲートを省略する。
- 変数用ビットはヒストグラムから除き、同じ公開ビット列の件数を合算する。事後選択の棄却ビットとは区別する。条件分岐は試行ごとに行い、その後で事後選択により選別する。表示の保存はその列までの事後選択の `if_test` 内で行う。確率・密度行列・ブロッホベクトルの表示は、その列までの事後選択を通過した試行だけの平均になる。
- WebGPU は固定された 1 回の測定結果を使うが、Aer は試行ごとに測定して分岐する。確率・密度行列・ブロッホベクトルは試行集団の平均になる。振幅表示だけは従来どおり `save_amplitudes` が返す 1 試行分の保存振幅を使い、試行平均でも混合状態の振幅でもない。

詳しくは[条件付きゲート](../../docs/web.md#条件付きゲート)を参照する。

## 事後選択

4 種類の事後選択 (`|0⟩⟨0|` / `|1⟩⟨1|` / `|+⟩⟨+|` / `|-⟩⟨-|`) と制御・反制御に対応する。
棄却条件を補助量子ビット `postselection_aux` に書き込み、専用の古典ビット `postselect_<n>` へ測定した後に補助量子ビットをリセットする。
結果 0 の試行だけを残すことで、制御部分空間への射影を `C` として `I - C + CP` を実現する。
X 基底では対象の前後に `H` を置く。対象そのものは測定せず、制御が不成立の分岐のコヒーレンスを保つ。

- `histogram` はすべての事後選択を通過した試行だけを数え、事後選択用の古典ビットを除く。`shots` は試行した回数のままで、ヒストグラムの合計は 0 の場合もある。
- 表示はその列までの成功条件を Aer の `if_test` で確認して保存する。後の事後選択は以前の表示に影響しない。ユニタリゲートと事後選択だけの回路では、残った試行の表示値は位相も含めて厳密な条件付き状態になる。
- 保存に到達した試行がなければ `defined: false` を返す。確率は全要素 `-1`、振幅・ブロッホベクトル・密度行列は 0 とし、密度行列の `unity` は `1e-12` とする。

制限は次のとおり。

- 補助量子ビットを 1 つ追加するため、Aer は `qubits + 1` 量子ビットを計算し、状態ベクトルのメモリ使用量は約 2 倍になる。API の量子ビット数は入力回路の数であり、補助量子ビットを含まない。
- 有限の試行数では低確率の成功を取り逃がし、可能な結果を未定義とする場合がある。
- Aer は標本抽出を行うため、WebGPU の受理確率 `1e-12` 以下をゼロとする閾値を厳密には再現しない。
- WebGPU は種を固定した 1 回の試行、Aer は試行ごとの測定を扱う。事後選択より前に測定があると、外部実行ではその集団を選別する。
- 生存率と全状態は返さない。画面の列ごとの増減率と `kept: N%` はローカル WebGPU の計算値である。

詳細は[事後選択の仕様](../../docs/postselection.md#外部-qiskit-実行)を参照する。

## Run

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner mock
```

CPU dev runner (requires a Python environment with `qiskit` and `qiskit-aer`):

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner qiskit-cpu-dev
```

GPU runner:

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m qni_qiskit_backend --port 4184 --runner qiskit-gpu
```

Production GPU-only runner:

```bash
QNI_QISKIT_ALLOWED_RUNNERS=qiskit-gpu \
PYTHONPATH=apps/qiskit-backend/src \
python3 -m qni_qiskit_backend --port 4184 --runner qiskit-gpu
```

## API

Health:

```bash
curl http://127.0.0.1:4184/health
```

Run:

```bash
curl -s http://127.0.0.1:4184/run \
  -H 'content-type: application/json' \
  -d '{"runner":"mock","qubits":4,"columns":[["H",1,1,1],["Amps1","Bloch","Probability","Density"]],"shots":128,"outputs":{"histogram":true,"amplitudes":[{"gate_id":2,"column":1,"span":1,"base_bit":3,"control_mask":0,"control_value":0,"phase_lock_enabled":true}],"bloch":[{"gate_id":3,"column":1,"wire":1,"control_mask":0,"control_value":0}],"probability":[{"gate_id":4,"column":1,"span":1,"base_bit":1,"control_mask":0,"control_value":0}],"densities":[{"gate_id":5,"column":1,"span":1,"base_bit":0,"control_mask":0,"control_value":0}]}}'
```

Response shape:

```json
{"status":"completed","runner":"mock","qubits":4,"shots":128,"histogram":{"0000":128},"truncated":false,"amplitudes":[{"gate_id":2,"span":1,"ket":[[1.0,0.0],[0.0,0.0]],"incoherent":[1.0,0.0],"quality":1.0,"phase_lock_index":0}],"bloch":[{"gate_id":3,"vector":[0.0,0.0,1.0]}],"probability":[{"gate_id":4,"span":1,"probabilities":[1.0,0.0]}],"densities":[{"gate_id":5,"span":1,"cells":[[1.0,0.0],[0.0,0.0],[0.0,0.0],[0.0,0.0]],"unity":1.0}]}
```

## Test

```bash
PYTHONPATH=apps/qiskit-backend/src python3 -m unittest discover apps/qiskit-backend/tests
```
