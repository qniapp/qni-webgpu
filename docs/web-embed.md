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

JavaScript は JavaScript の MIME 型、wasm は `application/wasm` で配信する。
HTTPS または localhost が必要。別オリジンから読み込む場合は全ファイルに CORS 許可を付ける。
wasm の URL は `import.meta.url` を基準に解決する。ホストページのパスや `<base>` に依存しない。
通常の `index.html` や `bootstrap.js` は埋め込みに使わない。

## JavaScript API

```js
import { startEmbed } from './assets/qni-embed.mjs'

const runner = await startEmbed(canvas, '{"cols":[["H"]]}', {
  showStatePanel: true,
})
// キャンバスを取り外す前に呼ぶ。複数回呼んでもよい。
runner.destroy()
```

`startEmbed(canvas: HTMLCanvasElement, circuit: string, settings?: {showStatePanel?: boolean, onProgress?: function})`
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
埋め込みでは URL の回路や実行モード、保存済み回路を読み込まない。
編集、消去、Undo / Redo はメモリ内だけで行い、URL と localStorage へ書き込まない。
回路ピッカー、保存操作、外部 GPU 実行への切り替えは表示しない。再読み込みで初期回路に戻る。

低水準の `qni-web.js` は初期化用の default export と、次の関数も公開する。

```js
start(canvas: HTMLCanvasElement): Promise<QniRunner>
start_embed(canvas: HTMLCanvasElement, circuit_json: string, show_state_panel: boolean): Promise<QniRunner>
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
H ゲートの計算、ストレージと履歴へのアクセス遮断、消去、破棄後の再起動、不正な回路の拒否を確認する。
GPU の読み戻しはテスト時だけ行う。

## 未対応事項

- パレットの制限とチュートリアル専用のレイアウトは未対応。
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
