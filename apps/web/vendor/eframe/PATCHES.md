# eframe 0.35.0 へのパッチ

このディレクトリは crates.io の `eframe-0.35.0.crate`
(sha256 `8dc8234e2f681b2afd2b2e8c332fa614de2fddbdcb28d0acf5c420449682ea90`、
上流のコミット `6f15dc0e16b26edce1fc2a05212eaf7e749c1d05`) を展開したもの。
crate 同梱の `Cargo.lock` だけを削除し、`apps/web/Cargo.toml` の `[patch.crates-io]` から参照している。

## 変更点

`WebOptions::text_agent_autofocus: bool` (既定値 `true`) を追加した。
`TextAgent::attach` はこの値を受け取り、IME / モバイルキーボード用の隠し `<input>` に設定する。
元のコードは常に `input.set_autofocus(true)` していた。

変更したファイル:

- `src/epi.rs`: フィールドと既定値の追加
- `src/web/web_runner.rs`: `TextAgent::attach` へ値を渡す
- `src/web/text_agent.rs`: `set_autofocus(true)` を引数の値に置き換える

## 理由

キャンバスをチュートリアル記事などに埋め込むと、ブラウザは読み込み時に autofocus の `<input>` (shadow root 内でも対象) へフォーカスし、ページをその位置までスクロールする。
初回表示でもリロードでも、記事の先頭や保存されていた位置ではなく埋め込み付近に移動してしまう。
qni-webgpu は埋め込みとして起動するときだけ `false` にする (`apps/web/src/lib.rs` の `start_runner`)。
単体アプリは `true` (既定値と同じ) を渡す。`<input>` 自体は残すので、IME とモバイルキーボードの入力は従来どおり動く。

## 上流への提案と削除の方針

- 上流の egui (emilk/egui) に同等の `WebOptions` 設定を提案する。
- 上流に取り込まれた版へ更新するときは、このディレクトリと `[patch.crates-io]` を削除し、上流側の設定へ移行する。
- それまでに eframe を更新する場合は、新しい版の crate を展開し直して同じ 3 か所を再適用する。
  `git log -p -- apps/web/vendor/eframe` で無改変の取り込みとパッチのコミットを分けてある。
