# WebGPU アプリの埋め込み

## ビルド

リポジトリルートで次を実行する。Rust の wasm32 ターゲット、Trunk、pnpm、GNU coreutils が必要。
初回は `pnpm -C apps/web install --frozen-lockfile` で依存関係を準備する。

```sh
bash apps/web/scripts/build-embed.sh
```

出力先は `apps/web/embed-dist/`。引数で別の出力先も指定できる。
このディレクトリの 4 ファイルをまとめて配信する。

- `qni-embed.mjs`: 埋め込み用の ESM 入口
- `qni-embed-error.mjs`: 起動原因の判定とエラー画面。wasm に依存しない
- `qni-web.js`: wasm-bindgen の JavaScript
- `qni-web_bg.wasm`: アプリ本体。フォントとシェーダも含む

埋め込みのビルドでは専用の Cargo プロファイル `embed` を使う。
`opt-level="z"`、fat LTO、`codegen-units=1`、`strip=true` と `wasm-opt -Oz` で配信サイズを抑える。
通常アプリの release プロファイルは変えない。wasm32 ターゲットの既定の `panic=abort` も変えない。
使っていない egui の既定フォントと WebGL 用の依存関係は無効にするが、
Geist、日本語の代替フォント、数学記号用の Hack、スクリーンリーダーは維持する。
この依存関係の削減は wasm32 ターゲットだけに適用し、その他のターゲットの描画と既定フォントの設定は従来のままにする。

JavaScript は JavaScript の MIME 型、wasm は `application/wasm` で配信する。
HTTPS または localhost が必要。別オリジンから読み込む場合は全ファイルに CORS 許可を付ける。
wasm の URL は `import.meta.url` を基準に解決する。ホストページのパスや `<base>` に依存しない。
通常の `index.html` や `bootstrap.js` は埋め込みに使わない。

## コミットに固定したリリース

`qniapp/qni-webgpu` は `master` への push ごとに埋め込み用のリリースを作成する。
`master` を選んだ手動実行にも対応し、ほかのブランチやプルリクエストからは公開しない。タグは `embed-<sha>` で、
`<sha>` は省略しない 40 桁のコミット SHA とする。

- リリース: `https://github.com/qniapp/qni-webgpu/releases/tag/embed-<sha>`
- ファイル: `https://github.com/qniapp/qni-webgpu/releases/download/embed-<sha>/<name>`

リリースとタグ、配布ファイルは上書きも削除もせず、無期限に保持する。
所有者の高宮さんがリポジトリの設定でリリースの不変性 (release immutability) を有効にしており、
公開後のファイル変更とタグの移動は GitHub が拒否する。
削除禁止と無期限保持は運用方針であり、この設定だけで削除権限をなくすものではない。
GitHub Pages の単体アプリや短期保存の CI 成果物とは別の配布経路である。

### 配布ファイル

すべて同じ階層に配置し、次の順で `manifest.json` に記録する。

1. `qni-web.js`
2. `qni-web_bg.wasm`
3. `qni-embed.mjs`
4. `qni-embed-error.mjs`
5. `h.svg` (`apps/web/assets/icons/h.svg`)
6. `design-system.css` (`docs/design-system/design-system.css`)
7. `h-gate.js` (`docs/design-system/components/h-gate.js`)
8. `LICENSE` (リポジトリ直下の MIT ライセンス)

さらに `manifest.json` と `manifest.json.sha256` を配布する。
`h-gate.js` はローカルファイルを読み込まず、SVG も内包するため、ほかのゲート部品は不要である。
同梱 CSS の色変数を使うので、`design-system.css` と組み合わせる。
CSS の Google Fonts への `@import` は外部依存として残る。フォントまで含む完全なオフライン配布ではない。
既に `h-gate` を登録しているホストは重複登録しないこと。
これらの表示用資産は SHA で固定するが、埋め込み API の互換性の契約には含めない。

### 検証用マニフェスト

`manifest.json` は UTF-8 の JSON で、以下の項目を持つ。

| 項目 | 内容 |
| --- | --- |
| `schema_version` | 形式の版。現在は整数 `1` |
| `source_commit` | ビルド元の 40 桁の小文字の 16 進 SHA |
| `embed_api_version` | ビルド済み `qni-embed.mjs` の `EMBED_API_VERSION` から取得する正の整数 |
| `built_at` | UTC の ISO 8601 日時 |
| `toolchain` | `rustc`、`wasm-bindgen`、`wasm-opt`、`trunk`、`node`、`pnpm` の版を表す文字列 |
| `files` | 上記 8 ファイルの配列。マニフェスト自身とそのチェックサムは含まない |
| `files[].name` | ディレクトリを含まない一意なファイル名 |
| `files[].sha256` | ファイル内容の SHA-256。小文字の 64 桁の 16 進数 |
| `files[].size` | バイト数の整数 |
| `files[].content_type` | `text/javascript`、`application/wasm`、`image/svg+xml`、`text/css`、`text/plain` のいずれか |

`manifest.json.sha256` は `sha256sum` 形式の 1 行
(`<SHA-256>  manifest.json` と末尾の改行) とする。
生成時にはビルド済みの `SOURCE_COMMIT` と指定 SHA の一致も確認し、不一致なら失敗する。
Rust は `rust-toolchain.toml` に従い、公開用ワークフローでは Trunk を `0.21.14` に固定する。
wasm-bindgen は `apps/web/Cargo.lock`、wasm-opt は `apps/web/Trunk.toml` の指定版を記録する。
残りのツールは `--version` の出力を記録する。版の記録は、別環境でのバイト単位の再現を保証しない。

利用側のリポジトリ (例: `qniapp/qni-tutorial-webgpu`) は次の順に検証してから、取得したファイルを自身の配信先へコピーする。
リリースの URL をブラウザから直接読み込む用途ではない。

1. `qni-webgpu.ref` などで期待する完全な SHA を固定し、そのリリースから
   `manifest.json` と `manifest.json.sha256` を取得する。
2. `sha256sum --check manifest.json.sha256` で、解析前のマニフェストのバイト列を検証する。
3. 下記のコマンドで `manifest.json` の来歴の証明を検証し、生成ワークフロー、`master` ブランチ、元コミットを照合する。
4. `schema_version`、期待する `source_commit`、対応する `embed_api_version` を確認する。
   ファイル名は上記の一覧と照合し、重複やディレクトリの指定を拒否する。
5. 各ファイルを同じリリースから取得し、`files` の SHA-256 とバイト数を照合する。
   各ファイルの来歴の証明も同じ条件で検証する。
   すべて成功した後だけ配信先に反映し、`LICENSE` も保持する。

チェックサムだけでは配布者の本人性を証明できない。
ワークフローは `actions/attest-build-provenance` で全配布ファイルのビルド来歴の証明を作成する。
`manifest.json` と取得した各ファイルについて次を実行する (例は wasm)。
証明のリポジトリ、生成ワークフロー、ブランチ、元コミットを指定して検証する。

```sh
sha="$(cat qni-webgpu.ref)"
gh attestation verify qni-web_bg.wasm -R qniapp/qni-webgpu \
  --signer-workflow qniapp/qni-webgpu/.github/workflows/embed-release.yml \
  --source-ref refs/heads/master \
  --source-digest "$sha"
```

### 公開手順と失敗時の扱い

公開には専用の GitHub App (Release App) のトークンを使い、`embed-*` タグはこの App だけが作成できるようタグのルールセットで保護し、所有者がリポジトリ変数 `EMBED_RELEASE_APP_CLIENT_ID` とシークレット `EMBED_RELEASE_APP_PRIVATE_KEY` を設定する。
Release App には意図的に Workflows 権限を与えないため、ワークフローは実行時点の `master` のコミットだけを公開し、古い SHA や `.github/workflows/` が現在の `master` と異なるコミットの再公開には対応しない。

ワークフローは最初に同名タグと、下書きを含む同名リリースの存在を確認する。
どちらかが存在すれば何も変更せず失敗する。認証や通信の失敗も「存在しない」とみなさない。
SHA ごとに実行を直列化し、進行中の実行は取り消さない。
下書きの一覧取得には書き込み権限が必要なため、存在確認は依存関係を実行しない専用のジョブで行う。
ビルドのジョブは読み取り権限だけを持ち、OIDC や書き込み用のトークンをビルド処理に渡さない。
検証済みファイルを CI 成果物として公開用のジョブへ渡し、そこで再検証して来歴の証明を作成する。

下書き作成の直前にも、認証済みの 404 応答と全ページのリリース一覧で同名タグ・リリースの不存在を再確認する。
存在しないタグ名と `--target` に完全な SHA を指定して下書きを作り、そのリリース ID を記録する。
ファイルの一括アップロードと公開操作のそれぞれの直前に、タグが存在せず、同名リリースが記録した ID の下書き 1 件だけであることを再確認する。
条件を満たさない場合や確認に失敗した場合は、その書き込みを行わず、人間の調査を待つ。
既存タグを検出した場合は、注釈付きタグも参照先のコミットまでたどり、元コミットとの一致・不一致をエラーに記録する。
公開前の確認では、既存タグが元コミットと一致していても失敗とする。`--target` では既存タグの参照先を変更できない。
Git タグを事前作成する命令は使わない。
タグの作成はリリース公開時の GitHub の処理に任せる。
下書きの間に全ファイルをアップロードし、一覧の名前とバイト数がローカルのマニフェストと一致することを確認してから、
`--draft=false --prerelease` でプレリリースとして公開する。
公開直後にタグの存在と元コミットへの一致を検証し、不一致や確認の失敗はエラーとする。追加の書き込みは行わない。
公開後は不変性の設定によりファイルを追加できないため、この順序を守る。
書き込み命令はタグ名で対象を選ぶ。各確認と書き込みは不可分ではないため、その間の競合までは防げない。
リリース本文には元コミットへのリンク、API の版、マニフェストの SHA-256 を記載する。

公開前に失敗した場合は下書きを残し、自動で削除・上書き・再公開しない。
再実行も存在確認で失敗する。残った下書きとファイル、失敗ログは人間が調査し、対応を判断する。
残った下書きから復旧する場合は、ワークフローのログを確認し、残った下書きリリースを削除してからワークフローを再実行する。
来歴の証明が作成できない場合も公開を止める。

ローカルで既存ビルドから検証する例 (出力先は空でなければならない):

```sh
export PATH="$HOME/.cargo/bin:$PATH"
sha="$(git rev-parse HEAD)"
QNI_SOURCE_COMMIT="$sha" bash apps/web/scripts/build-embed.sh
node apps/web/scripts/embed-release.mjs dry-run \
  --input apps/web/embed-dist --output release-stage --source-commit "$sha"
node apps/web/scripts/embed-release.mjs verify --output release-stage --source-commit "$sha"
```

`build --output release-stage --source-commit "$sha"` ならビルドから一括実行する。
`build` と `dry-run` では、指定 SHA のソースから作った信頼できるローカルのビルドを使い、
同梱する表示用資産と `LICENSE` もその SHA の内容であることを呼び出し側で確認する。
スクリプトの SHA 照合はビルド済みの識別情報との一致を確認するもので、ソースの真正性の証明ではない。
`dry-run` と `verify` は GitHub を変更しない。検証時も JavaScript を実行せず、識別情報を静的に読み取る。
`verify` の `--source-commit` を省略すると内部の整合性だけを確認するため、外部からの取得時には省略しない。
CI では既存の `web-build` の中でこの 2 つを実行する。

## 最適化ツールの固定

`apps/web/Trunk.toml` の `[tools] wasm_opt = "version_123"` で Binaryen 123 を指定する。
Trunk の既定の取得版も 123 だが、指定がないと PATH 上の別の版を優先してしまう。
指定版と異なるシステムの実行ファイルは選ばず、Trunk のキャッシュまたは取得した指定版を使う。
同じ版を名乗る PATH 上の実行ファイルや、明示的なコマンド行・環境変数による指定は別途確認する。
ビルドの詳細ログには選ばれた実行ファイル、版、引数を残す。

最適化の引数は `-Oz --enable-bulk-memory --enable-nontrapping-float-to-int`。
追加の 2 引数は Rust が出力する命令を許可するためのもので、`--all-features` は使わない。
wasm-bindgen の版は Cargo.lock の依存関係と合わせる。現在の確認済みの版は 0.2.129。

[wasm-bindgen #4228](https://github.com/wasm-bindgen/wasm-bindgen/issues/4228) は、
古いシステムの Binaryen による `WebAssembly.Table.grow` の失敗を報告している。
関連する [Binaryen #4711](https://github.com/WebAssembly/binaryen/issues/4711) の
誤ったテーブルのエクスポートは [#4736](https://github.com/WebAssembly/binaryen/pull/4736) で修正済みで、123 に含まれる。
この組み合わせでは通常アプリと埋め込みの最適化済みの成果物を実際の WebGPU ブラウザで起動し、
H ゲートの実行結果とエラーがないことを確認した。版や引数を変えたときも成果物をブラウザで確認する。

## 互換性の契約

公開成果物の `qni-embed.mjs` は `EMBED_API_VERSION = 1` と `SOURCE_COMMIT` を公開する。
前者は互換性の契約を表す整数、後者はビルド元の完全な 40 桁のコミット SHA である。
ビルド時は `QNI_SOURCE_COMMIT`、`GITHUB_SHA`、`git rev-parse HEAD` の順で値を選ぶ。
指定値が 40 桁の 16 進数でない場合と、開発用の `embed.mjs` を直接使う場合は `"unknown"` になる。
同じビルドの 4 ファイルを同じディレクトリへ配置し、異なるビルドのファイルを混ぜない。

契約の対象は以下に限る。引数、戻り値、既定値と操作の意味は本書の各節に従う。

- 定数: `EMBED_API_VERSION`、`SOURCE_COMMIT`。
- 入口: `startEmbed(canvas, circuit, settings)`。
- 設定: `palette` (省略・空配列・指定配列)、`maxWireCount`、`showStatePanel`、`showRestartButton`、`startupTimeoutMs`。
- 通知: `onProgress({stage, loaded, total})`、`onRestart()`、`onDeviceLost(error)`。
- ランナー: `selectStep(n)`、`selectedStep()`、`stepCount()`、`preferredHeight(width)`、`circuitJSON()`、`readStateVector()`、`destroy()`。
- エラー用の独立モジュール: `QniStartupError`、`diagnoseWebGpu()`、`renderStartupError()`、`STARTUP_ERROR_CODES`。
  これらは `qni-embed.mjs` からも再公開する。

`circuitJSON()` は **同期的に文字列を返す**。Promise は返さない。
`readStateVector()` はテスト専用の明示的な GPU 読み戻しで、描画には使わない。
`preferredHeight(width)` は繰り返し呼んでよく、250 ミリ秒ごとの問い合わせにも GPU 読み戻しは発生しない。

次の変更では `EMBED_API_VERSION` を増やす。

- 対象の公開名、メソッド、設定、通知の削除や改名。
- 引数・戻り値の型、同期・非同期の区別、既定値、ステップ番号の意味の変更。
- 既存のエラーコードの意味や通知の順序の変更。
- 受理する回路 JSON の形式を狭める変更。

新しい任意設定、メソッド、公開名の追加は互換性を保つ追加とする。
新しいエラーコードも追加できるため、ホストは未知のコードを一般的な失敗として扱わなければならない。
既存コードの意味を別の原因へ流用してはならない。表示文言や内部構造、描画の細かな寸法は契約に含めない。

`prepareEmbed` は任意の先読み用 API で、この契約の対象外である。
`classifyStartupError`、`preflightSync`、`detectPlatform`、`startupErrorContent`、低水準の `qni-web.js`、
Performance API のマーク、内部のテスト用フックも対象外である。
これらを利用する場合は、特定の `SOURCE_COMMIT` に固定し、そのビルドでの動作をホスト側で検証する。
特に大域関数 `read_state_vector()` は埋め込み用ではない。必ず `runner.readStateVector()` を使う。

### 対応端末

モバイルも正式な対応対象である。WebGPU が使える Android の Chrome、iPhone・iPad の Safari を含む。
デスクトップでは WebGPU が使える Windows / macOS の Chrome と Edge、macOS の Safari を対象とする。
端末やブラウザが WebGPU に対応しない場合も、`diagnoseWebGpu` と `renderStartupError` で
デスクトップと同じ原因別の診断・エラー画面を提供する。CPU や WebGL での代替計算は行わない。
Chromium のモバイル模擬環境で操作を検証するが、各 OS・ブラウザの実機検証の代わりにはならない。

## JavaScript API

```js
import { startEmbed } from './assets/qni-embed.mjs'

const runner = await startEmbed(canvas, '{"cols":[["H"]]}', {
  showStatePanel: true,
  palette: ['H', 'X'],
  maxWireCount: 1,
})
// キャンバスを取り外す前に呼ぶ。複数回呼んでもよい。
runner.destroy()
```

`startEmbed(canvas: HTMLCanvasElement, circuit: string, settings?: {showStatePanel?: boolean, palette?: string[], maxWireCount?: number, showRestartButton?: boolean, onRestart?: function, onProgress?: function, onDeviceLost?: function, startupTimeoutMs?: number})`
は `Promise<{destroy(): void, circuitJSON(): string, preferredHeight(widthCssPx: number): number, selectStep(n: number): void, selectedStep(): number | null, stepCount(): number, readStateVector(): Promise<Float32Array>}>` を返す。`showStatePanel` の既定値は `true`。
`onProgress` には `{stage, loaded, total}` を渡す。`stage` は `download`、`compile`、`gpu`、`prepare`。
`loaded` は展開後のバイト数。`total` は同一オリジンの非圧縮応答で長さが分かる場合だけ数値、それ以外は `null`。
圧縮応答の Content-Length から百分率を計算してはならない。
通知関数は例外を投げないようにする。

`prepareEmbed(): Promise<unknown>` を早めに呼ぶと、キャンバスの接続前に wasm の取得と
ストリーミングコンパイルを始められる。`startEmbed` と同じ初期化 Promise を共有するので重複取得しない。
失敗時は次の呼び出しで再試行する。応答のストリームを `Response` として wasm-bindgen に渡し、
`application/wasm` なら `WebAssembly.instantiateStreaming` を使う。ArrayBuffer へ事前に集めない。
読み込み段階は `qni:wasm-fetch-start`、`qni:wasm-fetch-end`、`qni:wasm-instantiated`、
`qni:runner-start`、`qni:runner-started` の Performance API のマークでも確認できる。
`wasm-fetch-end` はストリームがある場合だけ記録する。
DOM に接続済みで、幅と高さのあるキャンバスを渡す。shadow DOM 内でも使える。
ブラウザで WebGPU が使えなければ Promise が失敗する。WebGL や CPU への代替処理はない。
エラー表示の呼び出し (`renderStartupError`) と接続・切断の管理はホスト側で行う。

回路は URL と同じ Quirk 形式の JSON 文字列。列ごとの配列にゲート名を入れ、配列の位置が量子ビットを表す。
1 量子ビットへの H ゲートは `{"cols":[["H"]]}`。
明示的な初期化を付ける場合は `{"cols":[["|0>"],["H"]]}`。
空回路は `{"cols":[]}`。未知のゲート、不正な JSON、16 量子ビットを超える回路は拒否する。
qni と同じく、`["{ラベル"]` の列と `["}"]` の列で囲んだ列はコメントブロックとして枠で囲み、コメントを枠の上下にラベルとして表示する。
たとえば `{"cols":[["{量子もつれ"],["H"],["•","X"],["}"]]}`。ブロックの印の列はステップに数えず、シミュレーションにも影響しない。
コメントブロックをパレットから置いたりコメントを編集したりできるのは、パレットにコメントブロックがある場合だけである (「パレットの制限」を参照)。
ブロックの入れ子、ラベルのない `{`、対応する開始のない `}`、ほかのゲートと同じ列に置いた印は不正な回路として拒否する。
qni と同じく、ルートに文字列の `"title"` を付けられる (例: `{"cols":[["|0>"]],"title":"Superdense Coding"}`)。
キーの順序は問わない。タイトルは前後の空白を除いて保持し、編集後の `circuitJSON()` では空でなければ `cols` の後ろに書き出す。
埋め込みではタイトルを表示せず、ホストのページタイトルも変えない。`cols` と `title` 以外のキーを持つ回路と、文字列でない `title` は拒否する。
qni と同じく、`Measure>aliceX` は測定結果 (0 または 1) を変数 `aliceX` に書き込み、`X<aliceX` はその変数が 1 のときだけ X ゲートを適用する。
条件を付けられるのは `H`、`X`、`Y`、`Z`、`X^½`、`S`、`S†`、`T`、`T†`。
条件付きゲートは、それより左の列で最後にその変数へ書き込んだ測定の結果を読む。同じ列の測定は読まない。
一度も書き込まれていない変数は 0 とみなす。回路上では測定ゲートの上に変数名、条件付きゲートの上に `if 変数名` を表示し、適用されなかった条件付きゲートは灰色で描く。
変数名は回路ごとに独立している。
埋め込みでは URL の回路や実行モード、保存済み回路を読み込まない。
編集と Undo / Redo はメモリ内だけで行い、URL と localStorage へ書き込まない。
回路ピッカー、保存操作、外部 GPU 実行への切り替え、回路を全消去するゴミ箱ボタンは表示しない。
ツールバーには Undo と Redo だけが並ぶ。再読み込みで初期回路に戻る。

### ステップの選択

```js
const runner = await startEmbed(canvas, '{"cols":[["{量子もつれ"],["H"],["•","X"],["}"]]}')
runner.stepCount() // 2
runner.selectStep(1)
runner.selectedStep() // 1
const state = await runner.readStateVector() // CNOT を適用した直後の状態
```

`selectStep(n: number): void` は、回路のステップをクリックしたときと同じように固定選択を変更する。
`n` はブロックの印 (`"{ラベル"` と `"}"`) を除いた列の 0 始まりの番号。
初期化や表示ブロック、空の列も数えるが、エディタが右端に補う空の操作領域は数えない。
幅のある表示ブロックと後続列が重なる入力は読み込み時に後続列をずらすため、番号と列数はその調整後の回路を基準にする。
ステップ 0 は最初の列を適用した直後であり、適用前の初期状態ではない。
`n` は `-1 <= n < runner.stepCount()` を満たす整数でなければならない。
`-1` は最初の列より前の初期状態 `|0…0⟩` を選ぶ。小数、NaN、無限大、範囲外の値は `RangeError`、数値以外は `TypeError` を同期的に投げ、選択を変えない。
空回路では `-1` のみ選択できる。

`stepCount(): number` は現在の確定済み回路のステップ数を返す。
`selectedStep(): number | null` は固定選択の番号を返す。最初の列より前を選んだ場合は `-1`、空回路で `-1` 以外を選んでいる場合は `null` を返す。
UI で回路の右側の空の操作領域を選んだ場合は、表示される状態が同じである最終ステップの番号を返す。
埋め込みで回路の変更を確定した場合や Undo / Redo を実行した場合は、以前の固定選択を解除して最終状態を表示する。
ホバーによる一時表示は固定選択を変えない。ポインタが別のステップ上にある場合は、次の描画フレームから UI と同じ一時表示になる。

`readStateVector(): Promise<Float32Array>` は、選択や保留中の回路更新を GPU に反映してから表示中の状態を読み戻す。
ホバーによる一時表示中は、そのステップの状態を返す。
`selectStep(n)` の直後に呼んでも描画フレームを待つ必要はなく、そのステップの状態を取得できる。
戻り値は基底状態順の実部と虚部の組 `[re0, im0, re1, im1, ...]`。
`showStatePanel: false` でも使える。量子状態の計算は GPU のみで行い、読み戻しはこの API を明示的に呼んだときだけ行う。
ゲートのドラッグ中やローカル実行以外では読み戻しを拒否する。
ドラッグ中の `selectStep(n)` は確定済み回路の番号を選ぶ。ただし、回路の変更を確定すると、この選択も解除して最終状態を表示する。
回路が変わらなかった場合やドラッグを取り消した場合は固定選択を保つ。
読み戻す GPU バッファはランナーごとに独立している。複数の回路を同時に起動しても、
ほかの回路の選択や描画は `runner.readStateVector()` の結果に混ざらない。
1 個のランナーを破棄しても、ほかのランナーの選択と読み戻しは引き続き使える。

`destroy()` は何度呼んでもよい。破棄後はそれ以外の全メソッドが
`name: "Error"`、`message: "Circuit runner is destroyed"`、`code: "runner-destroyed"` のエラーになる。
同期メソッドは同期的に投げ、`readStateVector()` はこのエラーで Promise を拒否する。
破棄とデバイス喪失が重なった場合は破棄後のエラーを優先する。

### 再実行ボタン

`showRestartButton: true` と `onRestart` を両方渡すと、青い状態バーの右端、状態数の右隣に円形の矢印を表示する。
ボタンの読み上げ名とホバー時の説明は「量子回路を実行」。キーボードでも操作できる。
キャンバスにフォーカスした状態から Shift+Tab で再実行ボタンへ移り、Enter または Space で実行できる。
状態数の表示と操作領域を左へずらし、ボタンの領域ではパネルをドラッグしない。
狭い埋め込みでは状態数とボタンを優先し、両方のラベルが収まらないときは左の量子ビット数を省く。
状態数も収まらない幅では、状態数の左側をパネル内で切り取る。
`showRestartButton` の既定値は `false`。通知関数がない場合や `showStatePanel: false` の場合も表示しない。
通常アプリには表示しない。

クリックは引数なしで `onRestart()` に通知するだけで、埋め込み側は回路や状態を変更しない。
ホストが再実行の意味を決める。たとえば `runner.circuitJSON()` で編集中の回路を取り出し、
現在のランナーを破棄して同じキャンバスで起動し直せる。通知関数は例外を投げないようにする。
通知は描画処理が終わった後のマイクロタスクで届けるため、その中で回路の取得や破棄をしてよい。
通知前に破棄されたランナーやデバイスを失ったランナーからは通知しない。

新しいランナーには新しい測定用の乱数種を割り当て、測定結果を起動し直すたびに抽出し直す。
同じランナー内のステップ選択や読み戻しだけでは抽出し直さず、選択した位置の状態を比較できる。

低水準の `start_embed` を直接使う場合は、通知関数内で同じランナーを操作せず、
`queueMicrotask` でホスト側の処理を遅らせる。

### パレットの制限

`palette` にゲートの文字列の配列を渡すと、パレットにはそのゲートだけを渡した順に 1 行で並べる。
旧 Qni のチュートリアルの Liquid フィルタ `mini_qni` の引数と同じ書き方で、
`{{ json | strip | mini_qni: "|0>", "|1>", "H" }}` は `palette: ['|0>', '|1>', 'H']` になる。
文字列は回路 JSON のセルと同じ語彙で、`mini_qni` の使う `|0>`、`|1>`、`H`、`X`、`Y`、`Z`、`P`、`•`、`Bloch` のほか、
`Measure` や `Swap`、角度付きの `P(π/4)` も使える。角度のない `P` は旧 Qni と同じく π/2 で置かれる。
`QFT3` のような幅の指定、`Measure>a` や `X<a` のような測定変数と条件、未知の文字列は拒否する。
文字列 `"{}"` はコメントブロックを表す (例: `palette: ['H', '•', 'X', '{}']`)。
`"{}"` を含む埋め込みでは、パレットからコメントブロックを置けるほか、回路上のブロックのコメントの編集、左右のハンドルによる範囲の変更、ラベルのドラッグによる移動と削除ができる。
`"{}"` を含まない制限パレットでは、回路上のブロックは読むだけで、ラベルを押しても編集しない。既存のチュートリアルの埋め込みはこの扱いになる。

`palette` を省略すると従来どおり全ゲートのパレットを表示し、コメントブロックも表示ブロック欄の末尾 (3 列 × 2 行では右下) に並ぶ。
空配列 `[]` はパレットを表示しない。旧 Qni でパレットのない埋め込み (`mini_qni` の引数なしや `cnot_gate.html` など) に対応する。
制限したパレットが描画領域 (キャンバスから左右 8px ずつの余白を除いた領域) の左右に 16px を残した幅に 1 行で収まらない場合は、収まる最少の行数に折り返し、各行の個数をそろえる (354px 幅のキャンバスで 9 個なら 5 個と 4 個)。
パレットが 2 行より低ければ回路を上へ詰め、3 行以上に折り返して高くなれば回路を下げる。全ゲートのパレットも、幅が足りなければ折り返して回路を下げる (`docs/web.md`)。

### ワイヤー数の上限

`maxWireCount` に正の整数を渡すと、エディタが追加する空のワイヤーをその本数までに抑える。
旧 Qni の `<quantum-circuit>` 要素の `data-max-wire-count` 属性と同じ名前と意味で、
`data-max-wire-count="1"` は `maxWireCount: 1` になる。

- 回路の使う量子ビットが上限より少なくても、通常は最低 2 本のワイヤーを表示する。この最低本数を上限まで減らす。
  `maxWireCount: 1` なら 1 量子ビットの回路は 1 本だけ表示する。
- ゲートをドラッグしている間に下へ 1 本追加する空のワイヤーも、上限に達していれば追加しない。
  表示していないワイヤーにはゲートを置けない。範囲を変えられるゲートの下端を伸ばして新しいワイヤーを増やすこともできない。
- 回路がすでに上限より多くのワイヤーを使っていても、ワイヤーを削らず、回路を拒否もしない。
  旧 Qni のチュートリアルは 3 量子ビットのテレポーテーション回路などにも `data-max-wire-count="1"` を付けており、
  この場合はドラッグ中にワイヤーを追加しないことだけを意味する。

省略すると従来どおり最低 2 本を表示し、ドラッグ中はローカル実行の上限 (16 量子ビット) まで 1 本追加する。
0 以下、小数、数値でない値は起動前に拒否する。通常アプリにはこの設定がなく、動作は変わらない。

旧 Qni のチュートリアル (`apps/tutorial`) では、`data-min-wire-count` は常に `1`、
`data-max-wire-count` は `decrement_circuit.html` の 6 個の回路が `4`、
Liquid フィルタ `mini_qni` (`_plugins/mini_qni_filter.rb`) で作る回路が `2`、それ以外の直接書いた回路はすべて `1` である。

### 狭いキャンバスの回路

描画領域が 640px (Tailwind の `sm`) より狭い場合は、回路の左右の余白を詰める。
量子ビットのラベル (`q0:` など) を線の直前に右寄せし、線の始まりを描画領域の左端から 40px に置く。
右側の余白も 64px から 16px に減らす。
390px 幅のチュートリアルページ (キャンバス 354px) でも 5 列のゲートが切れずに収まる。
それより長い回路は、ホイールやトラックパッドの横スクロール、またはタッチの横スワイプで右側を表示する。
ゲートのない回路領域では、タップするとステップを選択し、横スワイプすると回路を横へ動かす。
ゲートやパレット項目から始めたドラッグはゲートの移動・配置として扱う。
空白からの縦スワイプは記事の縦スクロールへ渡す。ホストは `preferredHeight` で必要な高さを確保し、
キャンバスの CSS に `touch-action: none` を指定してはならない (記事のスクロールを妨げるため)。
描画領域が 640px 以上なら配置は変わらない。

### 初期表示と状態パネルの配置

埋め込みでは、回路を読み込んだ直後に固定選択をステップ 0 に置き、最初の列を適用した状態を表示する。
旧 Qni のチュートリアルと同じで、`{"cols":[["|0>","|0>"],["H"],["•","X"]]}` は |00> から始まる。
ゲートの追加・削除などで回路の変更を確定した後や、Undo / Redo の後は、回路全体を適用した最終状態を表示する。
通常アプリの起動時と Undo / Redo の後は従来どおり最終状態を表示し、変更を確定しても固定選択を保つ。

状態パネルが回路やゲートを隠さないように、埋め込みでは回路の下端 (最後の量子ビットのステップ表示とブロックのラベル) より下にパネルを置く。
通常アプリと同じ下寄せの位置で回路と重ならなければその位置のままにする。
重なる場合は回路の 16px 下へ移し、キャンバスの下端から 16px までの高さに収まるようビューポートを縮める。
最小の高さ 80px でも収まらないときは、パネルの下側をキャンバスの外へはみ出させる。
パネルの幅はキャンバスの幅から左右 16px ずつを引いた幅までに抑える。

### 低水準の API

低水準の `qni-web.js` は初期化用の default export と、次の関数も公開する。

```js
start(canvas: HTMLCanvasElement): Promise<QniRunner>
start_embed(canvas: HTMLCanvasElement, circuit_json: string, show_state_panel: boolean, palette: string[] | undefined, max_wire_count: number | undefined, show_restart_button: boolean, on_restart?: function): Promise<QniRunner>
```

直接使う場合は先に wasm を初期化する必要がある。
`QniRunner` にも `selectStep(n)`、`selectedStep()`、`stepCount()` があり、番号の意味と数値検証は同じ。
低水準の空回路の `selectedStep()` は `undefined` を返す。回路の取得は `circuit_json()`、
非同期の読み戻しは `read_state_vector()` を使う。
高さの取得は `preferred_height(width_css_px)` を使う。
`QniRunner.destroy()` は `WebRunner::destroy()` を呼ぶ。低水準のハンドルは最後に `free()` も呼ぶ。
`startEmbed` の返すハンドルは `destroy()` と `free()` の両方を処理する。
通常アプリの起動、URL 同期、ローカル保存は従来どおり。

### 自動高さ

`runner.preferredHeight(canvas.clientWidth)` は、キャンバスに必要な高さを CSS ピクセルで返す。
正の有限幅を渡す。破棄後の呼び出しはエラーになる。
低水準 API の名前は `QniRunner.preferred_height(width_css_px)`。

実際のパレットの折り返し、ワイヤー間隔、コメントブロックの上下余白、状態パネルの表示設定と
指定された表示領域の高さを使って計算する。前の描画の高さやスクロール位置には依存せず、
GPU の値の読み戻しも行わない。追加可能な場合は次の空ワイヤーを 1 本分確保するため、
ドラッグ開始時にページが動かない。`maxWireCount` は追加ワイヤーを制限するが、既存のワイヤーは削らない。

状態パネル表示時は、回路下端 + 間隔 16px + 見出し 32px + 表示領域の高さ + 下余白 64px を
基本とし、回路のスクロール領域が収まる高さとの大きい方に中央パネルの下余白 8px を加える。
非表示時も回路のスクロール領域の下余白 16px を確保する（描画下端からは 28px）。
単位は CSS ピクセルで、外枠や外部リンクの高さは含まない。
幅変更や回路編集の後に再度呼び出す。自動通知はないため、必要なら定期的に問い合わせる。

これはキャンバス内の縦スクロールを避けるための高さであり、長い回路の横方向の移動や、
状態パネル内のパン・ズームを無効にはしない。状態ベクトル全体を自然寸法で展開すると
大きな回路では数千ピクセルになるため、指定された表示領域を保ち、埋め込みでは初期の円グリッド全体をその内側に縮小して収める。
操作によるズーム後のパンは引き続き利用できる。狭い幅では冗長な量子ビット数の見出しを省略する。

```js
const height = runner.preferredHeight(canvas.clientWidth)
canvas.style.height = `${height}px`
```

## 起動エラーの表示

`startEmbed` の失敗は `QniStartupError` で通知する。`code` は原因、`message` は元の説明、
`cause` は元の例外、`detail` は HTTP 状態などの補足情報を持つ。
`onDeviceLost(error)` にも `gpu-device-lost` のエラーを渡す。

`STARTUP_ERROR_CODES` は次のコードを並べた凍結済みの配列である。

| コード | 意味 | 見出し |
| --- | --- | --- |
| `insecure-context` | 安全でない接続 | 安全な接続で開いてください |
| `wasm-unavailable` | WebAssembly が利用不能 | ブラウザーの設定で止められています |
| `webgpu-unsupported` | WebGPU API が利用不能 | このブラウザーでは動きません |
| `gpu-adapter-unavailable` | アダプタ取得が失敗、または結果が null | GPU を使えませんでした |
| `gpu-device-unavailable` | GPU デバイスの取得・準備が失敗 | GPU の準備に失敗しました |
| `gpu-device-lost` | 共有 GPU デバイスを喪失 | GPU との接続が切れました |
| `load-failed` | モジュール・wasm の取得、コンパイル、リンクが失敗 | 読み込みに失敗しました |
| `startup-timeout` | GPU の起動待ちが指定期限を超過 | 起動が止まってしまいました |
| `invalid-embed` | 不正な回路・設定、または起動中のキャンバス切断 | 回路のデータに問題があります |
| `unknown` | 分類できない失敗 | 量子回路を起動できませんでした |

```js
let errors

async function start() {
  let started = false
  try {
    errors ??= await import('./assets/qni-embed-error.mjs')
    const { startEmbed } = await import('./assets/qni-embed.mjs')
    const runner = await startEmbed(canvas, circuit, {
      startupTimeoutMs: 20_000,
      onDeviceLost: error => {
        if (started) errors.renderStartupError(errorSlot, error, { onRetry: start })
      },
    })
    started = true
    return runner
  } catch (error) {
    if (errors) errors.renderStartupError(errorSlot, error, { onRetry: start })
    else errorSlot.textContent = '量子回路を起動できませんでした。ページを再読み込みしてください。'
  }
}
```

実際のホストでは、再試行の前に前のランナーを破棄し、エラー表示を消してから起動する。
`diagnoseWebGpu()` は wasm を取得せずに環境を確認し、原因か `null` を返す。
`startEmbed` 自体も同じ事前確認を行う。`qni-embed.mjs` の取得に失敗した場合も、
独立した `qni-embed-error.mjs` を読み込めていればエラー画面を表示できる。
このモジュールまで取得できない場合は、ホスト側で再読み込みを案内する文字を表示する。

`startupTimeoutMs` はアダプタの確認とランナーの GPU 初期化それぞれの期限で、既定は 20 秒、`0` で無効になる。
wasm のダウンロードとコンパイルは期限に含めない。
単独で呼ぶ `diagnoseWebGpu()` には期限がない。期限付きの起動には `startEmbed` を使う。
時間切れの後は同じ初期化を待ち続ける可能性があるため、再試行ではなくページの再読み込みを案内する。
遅れて起動したランナーは破棄する。

`renderStartupError(container, error, options)` は指定要素の中身を置き換え、専用の shadow DOM に描画する。
`container` 自体が shadow DOM 内にあっても使える。枠の幅と高さはホスト側で決める。
狭い枠では配置を切り替え、低い枠では内部をスクロールする。フォントの追加取得は行わない。
ホストの `--qni-startup-error-font` で本文の書体を、`--qni-startup-error-mono-font` で「エラーの詳細」などの等幅の書体を指定できる。
`options.onRetry` を省くと、再試行の代わりにページの再読み込みを案内する。
`options.headingLevel` は `2`、`3`、`4` から選べる (既定は `3`)。
戻り値の `code` は原因、`naturalHeight()` は内容に必要な高さ、`remove()` は表示の削除に使う。

[表示の確認ページ](design-system/embed-startup-error.html)で各原因の画面を確認できる。
リポジトリ直下で `python3 -m http.server 4194 --bind 127.0.0.1` を起動し、
`http://127.0.0.1:4194/docs/design-system/embed-startup-error.html` を開く。

### チュートリアルからの利用

別リポジトリ `qniapp/qni-tutorial-webgpu` では、依存コミットを記録する `qni-webgpu.ref` を更新し、
対応する `embed-<sha>` リリースから「検証用マニフェスト」の手順でファイルを取得・検証する。
埋め込み要素の `qni-webgpu-circuit.ts` にある独自の WebGPU 判定とエラー文をこの API に置き換える。
属性の検証失敗は `invalid-embed` として渡し、再試行時にはエラー表示と原因属性を消す。
テストの GPU の代用品にも `requestAdapter` を用意し、原因別の表示と再試行を検証する。

## カスタム要素への接続

埋め込み API はキャンバスを起動する。カスタム要素は自動登録しない。
ホスト側で旧 Qni と異なる名前 (例: `qni-webgpu-circuit`) を登録し、shadow DOM にキャンバスを作る。
`connectedCallback` で起動し、`disconnectedCallback` で破棄する。
起動待ちの間にキャンバスが切断された場合、起動処理は接続状態を確認し、作成済みのハンドルがあれば破棄して
`invalid-embed` (`Embed canvas is disconnected`) で拒否する。ホストは起動の Promise の拒否を必ず処理する。
切断や再接続に伴う古い起動の拒否は画面にエラーを表示せずに処理し、新しい起動の表示を上書きしない。
起動が先に完了した場合も、切断時にそのハンドルを破棄する。
再接続時には新しいハンドルを作り、前の起動処理の完了を待ってから同じキャンバスを使う。

## 入力用 `<input>` の autofocus

eframe 0.35.0 は IME とモバイルキーボード用の隠し `<input>` に常に `autofocus` を付ける。
記事に埋め込むと、ブラウザが読み込み時にこの `<input>` へフォーカスし、ページを埋め込み付近までスクロールしてしまう。
そこで `apps/web/vendor/eframe` に取り込んだ eframe に `WebOptions::text_agent_autofocus` を追加し、埋め込みとして起動するときだけ `false` にした。
単体アプリは `true` (eframe の既定と同じ) のまま。
パッチの内容と上流への提案方針は `apps/web/vendor/eframe/PATCHES.md` に記録している。

## 検証

```sh
bash apps/web/scripts/build-embed.sh
pnpm -C apps/web exec playwright test tests/embed.spec.ts tests/embed-contract.spec.ts \
  tests/embed-mobile-contract.spec.ts tests/embed-startup-error.spec.ts --workers=1
```

`pnpm -C apps/web run test:pw-legacy` と `pnpm -C apps/web test` は、埋め込み用の成果物を毎回ビルドしてから全テストを実行する。
Linux でディスプレイがない場合は `xvfb-run -a` で Playwright を実行する。
テストは別オリジンのネストしたパスからモジュールを読み込み、shadow DOM の描画、
H ゲートの計算、ストレージと履歴へのアクセス遮断、消去、破棄後の再起動、不正な回路の拒否、
ステップ 0 の初期表示、制限したパレットからの配置、不正なパレットの拒否、960x560 で状態パネルが回路を隠さないこと、
354x592 でパレットの折り返しと末尾のゲートが切れないこと、折り返した 2 行目のゲートを配置できること、
`maxWireCount: 1` で 1 本のワイヤーだけを表示し、ドラッグ中も 2 本目を追加せず置けないこと、不正な `maxWireCount` の拒否を確認する。
ブロックの印を除いたステップの選択、H と CNOT の各ステップの選択直後の読み戻し、
状態パネル非表示での選択、不正な番号と空回路の選択拒否も確認する。
UI のクリックとの一致、ホバー表示、低水準の API、描画前の編集結果の読み戻し、容量エラーの保持、破棄後の選択拒否も検証する。
ステップ選択の前後で必要な高さが変わらないことと、破棄後の高さ取得の拒否も確認する。
入力用 `<input>` に autofocus が付かないことと、長い記事の下に埋め込んでもページが先頭のままであることを確認する。
角度エディタへの文字入力が `<input>` を通って回路に反映されること、単体アプリでは autofocus が残ることも確認する。
契約のテストでは、成果物を実際の別 HTTP サーバから CORS 付きで配信し、
入れ子のパスからの読み込み、ビルド元の SHA、同期の回路取得、11 個の異なる回路の同時起動と独立した読み戻しを検証する。
共有デバイスの喪失通知を模擬する検査に加え、実際の `GPUDevice.destroy()` による喪失と新しいデバイスでの復旧も検証する。
390px・DPR 2 のモバイル模擬環境では、タップ、横スワイプ、ゲートのドラッグ、記事の縦スクロールを検証する。
GPU の読み戻しは明示的な検証時だけ行う。

## 共有 GPU の喪失と復旧

同じモジュールから起動した埋め込みは GPU デバイスを共有する。
共有デバイスが失われると、ページの `qni-device-lost` 通知を受けた、まだ破棄されていない全ランナーが
`onDeviceLost(error)` を 1 回ずつ呼ぶ。起動中も通知の対象で、起動の Promise も `gpu-device-lost` で拒否する。
通知を受けたランナー同士の呼び出し順には依存しない。破棄済みのランナーは通知しない。
`onDeviceLost` は同期的に呼ばれ、まだ届いていない `onRestart` は取り消される。
起動中の切断とデバイス喪失が重なった場合、起動の拒否では `gpu-device-lost` を優先する。
すでに届いた `onRestart` を取り消すことはない。

喪失後も `circuitJSON()` は同期的に現在の確定済み回路を返し、`destroy()` は使える。
それ以外のメソッドは `name: "Error"`、`message: "Qni GPU device lost"`、`code: "gpu-device-lost"` のエラーになる。
`readStateVector()` では Promise を拒否する。
ホストは各ランナーの `circuitJSON()` を保存して `destroy()` し、その文字列で `startEmbed` を呼び直す。
実際のデバイス喪失時には共有デバイスの保持を解除し、次の起動で新しいデバイスを準備する。
回路は保持するが、測定の標本結果・選択位置・編集履歴は引き継がない。

## 契約外の共有状態

大域関数 `read_state_vector()` は契約外の診断用 API で、`apps/web/src/lib.rs` から
`apps/web/src/gpu/readback.rs` の `GPU_READBACK` を通じて読み戻す。
このモジュールで最後に GPU の描画準備・状態更新を行ったランナーの登録済みバッファと状態数を使う。
明示的なランナーの読み戻しも登録を更新する。
最終状態または選択列のバッファを指すが、登録時点の複製ではなく、保留中の選択を同期しない。
埋め込みが同居する場合はそのバッファを指すこともあるため、埋め込みでは `runner.readStateVector()` を使う。

ブロッホベクトルなどの診断用 GPU ハンドルとスロット一覧も大域的で、契約の対象外である。
これらは最後に更新したランナーを指し、特定の埋め込みの状態を識別できない。
一方、SVG アイコンのテクスチャと SDF アイコンの描画先形式は egui コンテキストごとに保持する。
契約の `runner.readStateVector()` はランナー自身の GPU バッファを使う。

最後のランナーを破棄しても、`apps/web/src/lib.rs` の `SHARED_GPU` は
`SharedGpuInitialization::Ready` のまま、GPU のインスタンス、アダプタ、デバイス、キューを次の起動用に保持する。
大域的な診断用ハンドルも破棄後のバッファを保持し得るため、`destroy()` は完全な GPU メモリ解放を保証しない。
回路ライブラリ、テストフック、外部実行の通信状態も共有されるが、埋め込みではこれらを使わない。
