# WebGPU アプリの埋め込み

## ビルド

リポジトリルートで次を実行する。Rust の wasm32 ターゲット、Trunk、pnpm、GNU coreutils が必要。
初回は `pnpm -C apps/web install --frozen-lockfile` で依存関係を準備する。

```sh
bash apps/web/scripts/build-embed.sh
```

出力先は `apps/web/embed-dist/`。引数で別の出力先も指定できる。
このディレクトリの 3 ファイルをまとめて配信する。

- `qni-embed.mjs`: 埋め込み用の ESM 入口
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

`startEmbed(canvas: HTMLCanvasElement, circuit: string, settings?: {showStatePanel?: boolean, palette?: string[], maxWireCount?: number, onProgress?: function})`
は `Promise<{destroy(): void}>` を返す。`showStatePanel` の既定値は `true`。
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
エラー表示と接続・切断の管理はホスト側で行う。

回路は URL と同じ Quirk 形式の JSON 文字列。列ごとの配列にゲート名を入れ、配列の位置が量子ビットを表す。
1 量子ビットへの H ゲートは `{"cols":[["H"]]}`。
明示的な初期化を付ける場合は `{"cols":[["|0>"],["H"]]}`。
空回路は `{"cols":[]}`。未知のゲート、不正な JSON、16 量子ビットを超える回路は拒否する。
qni と同じく、`["{ラベル"]` の列と `["}"]` の列で囲んだ列はラベル付きのブロックとして枠で囲んで表示する。
たとえば `{"cols":[["{量子もつれ"],["H"],["•","X"],["}"]]}`。ブロックの印の列はステップに数えず、シミュレーションにも影響しない。
ブロックの入れ子、ラベルのない `{`、対応する開始のない `}`、ほかのゲートと同じ列に置いた印は不正な回路として拒否する。
qni と同じく、ルートに文字列の `"title"` を付けられる (例: `{"cols":[["|0>"]],"title":"Superdense Coding"}`)。
キーの順序は問わない。タイトルは前後の空白を除いて保持し、編集後の `circuitJSON()` では空でなければ `cols` の後ろに書き出す。消去するとタイトルも消える。
埋め込みではタイトルを表示せず、ホストのページタイトルも変えない。`cols` と `title` 以外のキーを持つ回路と、文字列でない `title` は拒否する。
qni と同じく、`Measure>aliceX` は測定結果 (0 または 1) を変数 `aliceX` に書き込み、`X<aliceX` はその変数が 1 のときだけ X ゲートを適用する。
条件を付けられるのは `H`、`X`、`Y`、`Z`、`X^½`、`S`、`S†`、`T`、`T†`。
条件付きゲートは、それより左の列で最後にその変数へ書き込んだ測定の結果を読む。同じ列の測定は読まない。
一度も書き込まれていない変数は 0 とみなす。回路上では測定ゲートの上に変数名、条件付きゲートの上に `if 変数名` を表示し、適用されなかった条件付きゲートは灰色で描く。
変数名は回路ごとに独立している。外部 GPU 実行は測定変数と条件付きゲートに対応しない。
埋め込みでは URL の回路や実行モード、保存済み回路を読み込まない。
編集、消去、Undo / Redo はメモリ内だけで行い、URL と localStorage へ書き込まない。
回路ピッカー、保存操作、外部 GPU 実行への切り替えは表示しない。再読み込みで初期回路に戻る。

### パレットの制限

`palette` にゲートの文字列の配列を渡すと、パレットにはそのゲートだけを渡した順に 1 行で並べる。
旧 Qni のチュートリアルの Liquid フィルタ `mini_qni` の引数と同じ書き方で、
`{{ json | strip | mini_qni: "|0>", "|1>", "H" }}` は `palette: ['|0>', '|1>', 'H']` になる。
文字列は回路 JSON のセルと同じ語彙で、`mini_qni` の使う `|0>`、`|1>`、`H`、`X`、`Y`、`Z`、`P`、`•`、`Bloch` のほか、
`Measure` や `Swap`、角度付きの `P(π/4)` も使える。角度のない `P` は旧 Qni と同じく π/2 で置かれる。
`QFT3` のような幅の指定、`Measure>a` や `X<a` のような測定変数と条件、未知の文字列は拒否する。

`palette` を省略すると従来どおり全ゲートのパレットを表示する。
空配列 `[]` はパレットを表示しない。旧 Qni でパレットのない埋め込み (`mini_qni` の引数なしや `cnot_gate.html` など) に対応する。
制限したパレットが描画領域 (キャンバスから左右 8px ずつの余白を除いた領域) の左右に 16px を残した幅に 1 行で収まらない場合は、収まる最少の行数に折り返し、各行の個数をそろえる (354px 幅のキャンバスで 9 個なら 5 個と 4 個)。
パレットが全ゲートのパレット (2 行) より低ければ回路を上へ詰め、3 行以上に折り返して高くなれば回路を下げる。

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
それより長い回路は、従来どおりホイールやトラックパッドの横スクロールで右側を表示する。
描画領域が 640px 以上なら配置は変わらない。

### 初期表示と状態パネルの配置

埋め込みでは、回路を読み込んだ直後にブレークポイントをステップ 0 に置き、最初の列を適用した状態を表示する。
旧 Qni のチュートリアルと同じで、`{"cols":[["|0>","|0>"],["H"],["•","X"]]}` は |00> から始まる。
Undo / Redo で回路を読み込み直したときもステップ 0 に戻る。通常アプリは従来どおり最終状態を表示する。

状態パネルが回路やゲートを隠さないように、埋め込みでは回路の下端 (最後の量子ビットのステップ表示とブロックのラベル) より下にパネルを置く。
通常アプリと同じ下寄せの位置で回路と重ならなければその位置のままにする。
重なる場合は回路の 16px 下へ移し、キャンバスの下端から 16px までの高さに収まるようビューポートを縮める。
最小の高さ 80px でも収まらないときは、パネルの下側をキャンバスの外へはみ出させる。
パネルの幅はキャンバスの幅から左右 16px ずつを引いた幅までに抑える。

### 低水準の API

低水準の `qni-web.js` は初期化用の default export と、次の関数も公開する。

```js
start(canvas: HTMLCanvasElement): Promise<QniRunner>
start_embed(canvas: HTMLCanvasElement, circuit_json: string, show_state_panel: boolean, palette?: string[], max_wire_count?: number): Promise<QniRunner>
```

直接使う場合は先に wasm を初期化する必要がある。
`QniRunner.destroy()` は `WebRunner::destroy()` を呼ぶ。
低水準のハンドルは最後に `free()` も呼ぶ。`startEmbed` の返すハンドルは両方を処理する。
通常アプリの起動、URL 同期、ローカル保存は従来どおり。

## カスタム要素への接続

埋め込み API はキャンバスを起動する。カスタム要素は自動登録しない。
ホスト側で旧 Qni と異なる名前 (例: `qni-webgpu-circuit`) を登録し、shadow DOM にキャンバスを作る。
`connectedCallback` で起動し、`disconnectedCallback` で破棄する。
起動待ちの間に切断された場合も、Promise が完了したらそのハンドルを破棄する。
再接続時には新しいハンドルを作り、前の起動処理の完了を待ってから同じキャンバスを使う。

## 検証

```sh
bash apps/web/scripts/build-embed.sh
pnpm -C apps/web exec playwright test tests/embed.spec.ts --workers=1
```

`pnpm -C apps/web run test:pw-legacy` と `pnpm -C apps/web test` は、埋め込み用の成果物を毎回ビルドしてから全テストを実行する。
Linux でディスプレイがない場合は `xvfb-run -a` で Playwright を実行する。
テストは別オリジンのネストしたパスからモジュールを読み込み、shadow DOM の描画、
H ゲートの計算、ストレージと履歴へのアクセス遮断、消去、破棄後の再起動、不正な回路の拒否、
ステップ 0 の初期表示、制限したパレットからの配置、不正なパレットの拒否、960x560 で状態パネルが回路を隠さないこと、
354x592 でパレットの折り返しと末尾のゲートが切れないこと、折り返した 2 行目のゲートを配置できること、
`maxWireCount: 1` で 1 本のワイヤーだけを表示し、ドラッグ中も 2 本目を追加せず置けないこと、不正な `maxWireCount` の拒否を確認する。
GPU の読み戻しはテスト時だけ行う。

## 未対応事項

- TODO: ページ内で GPUDevice を共有する。現状はランナーごとにデバイスを作る。
- TODO: デバイス喪失への復旧を追加する。
- TODO: 複数の埋め込みで使うスレッドローカルの状態をインスタンスごとに分ける。
  `apps/web/src/icons/svg_icon.rs` の `TEXTURE_CACHE` は別の egui コンテキストのテクスチャを共有してしまう。
  `apps/web/src/icons/sdf_icon.rs` の描画先形式も共有され、異なる形式では衝突する。
  `apps/web/src/gpu/readback.rs` の GPU ハンドルとスロット一覧は最後に描画したランナーを指す。
  これらのハンドルは破棄後も最後の GPU リソースを保持するため、破棄は完全な GPU メモリ解放を保証しない。
  回路ライブラリとテストフックの状態、外部実行の通信状態も共有される。
  埋め込みでは通常アプリのフックと外部実行を使わないが、複数同時配置の保証はまだない。
  テスト専用の読み戻し関数はランナーを指定できないので、検証対象は 1 個ずつにする。

対応対象は WebGPU が使える Windows / macOS の Chrome と Edge、macOS の Safari。
すべての対象ブラウザでの実機検証は別途必要。
