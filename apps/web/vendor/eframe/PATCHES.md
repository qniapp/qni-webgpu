# eframe 0.35.0 へのパッチ

このディレクトリは crates.io の `eframe-0.35.0.crate`
(sha256 `8dc8234e2f681b2afd2b2e8c332fa614de2fddbdcb28d0acf5c420449682ea90`、
上流のコミット `6f15dc0e16b26edce1fc2a05212eaf7e749c1d05`) を展開したもの。
crate 同梱の `Cargo.lock` だけを削除し、`apps/web/Cargo.toml` の `[patch.crates-io]` から参照している。

## TextAgent の autofocus

### 変更点

`WebOptions::text_agent_autofocus: bool` (既定値 `true`) を追加した。
`TextAgent::attach` はこの値を受け取り、IME / モバイルキーボード用の隠し `<input>` に設定する。
元のコードは常に `input.set_autofocus(true)` していた。

変更したファイル:

- `src/epi.rs`: フィールドと既定値の追加
- `src/web/web_runner.rs`: `TextAgent::attach` へ値を渡す
- `src/web/text_agent.rs`: `set_autofocus(true)` を引数の値に置き換える

### 理由

キャンバスをチュートリアル記事などに埋め込むと、ブラウザは読み込み時に autofocus の `<input>` (shadow root 内でも対象) へフォーカスし、ページをその位置までスクロールする。
初回表示でもリロードでも、記事の先頭や保存されていた位置ではなく埋め込み付近に移動してしまう。
qni-webgpu は埋め込みとして起動するときだけ `false` にする (`apps/web/src/lib.rs` の `start_runner`)。
単体アプリは `true` (既定値と同じ) を渡す。`<input>` 自体は残すので、IME とモバイルキーボードの入力は従来どおり動く。

## 高 DPR でのキャンバス寸法

`src/web/events.rs` の `get_display_size` は、`ResizeObserverEntry.contentRect` の CSS 寸法に `native_pixels_per_point()` を掛けてから丸め、描画バッファの寸法を決める。
`devicePixelContentBoxSize` は使用しない。Chrome の DPR エミュレーションでは、この値が `window.devicePixelRatio` と一致しない倍率を使う場合がある。
たとえば CSS 幅 960px、DPR 2 でも幅 960 を返すと、従来の処理は描画バッファを 960px にしていた。
egui はこの幅を DPR 2 で割るため論理幅が 480pt になり、960 CSS px の表示領域では内容が 2 倍に拡大される。

修正後は egui と描画バッファが同じ倍率を使う。
CSS 寸法を先に整数へ丸めず、DPR を掛けた後で丸めるため、小数の CSS 寸法も描画バッファへ反映される。
ホスト側が `canvas.width` / `canvas.height` を CSS 寸法 × DPR に設定する場合と、eframe だけが設定する場合の両方を `tests/embed.spec.ts` で検証する。
チュートリアル側の寸法設定を変更する必要はない。

## 上流への提案と削除の方針

- 上流の egui (emilk/egui) に同等の `WebOptions` 設定とキャンバス寸法の修正を提案する。
- 両方の変更が上流に取り込まれた版へ更新するときは、このディレクトリと `[patch.crates-io]` を削除し、上流側の設定へ移行する。
- それまでに eframe を更新する場合は、新しい版の crate を展開し直して autofocus 設定とキャンバス寸法の変更を再適用する。
  `git log -p -- apps/web/vendor/eframe` で無改変の取り込みとパッチのコミットを分けてある。
