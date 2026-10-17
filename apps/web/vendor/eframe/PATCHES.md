# eframe 0.35.0 へのパッチ

このディレクトリは crates.io の `eframe-0.35.0.crate`
(sha256 `8dc8234e2f681b2afd2b2e8c332fa614de2fddbdcb28d0acf5c420449682ea90`、
上流のコミット `6f15dc0e16b26edce1fc2a05212eaf7e749c1d05`) を展開したもの。
crate 同梱の `Cargo.lock` だけを削除し、`apps/web/Cargo.toml` の `[patch.crates-io]` から参照している。

## 変更点 1: 入力用 `<input>` の autofocus を切り替える

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

## 変更点 2: タッチの押下を二重に積まない

`install_pointerdown` は `pointerType` が `"touch"` の `pointerdown` を無視する。

変更したファイル:

- `src/web/events.rs`: `install_pointerdown` の先頭で `event.pointer_type() == "touch"` なら、親要素への伝播だけを止めて戻る

### 理由

指でタップすると、ブラウザは `pointerdown` と `touchstart` の両方を送る。
元のコードはどちらからも押下イベント `PointerButton { pressed: true }` を egui の入力キューに積み、さらに `pointerdown` の中で `runner.logic()` を呼ぶ。
このため 1 回のタップが、別々のフレームで 2 回の押下として届いていた。
縦レイアウトのパレットでは、`›` を 1 回タップすると 2 ページ進んでいた。
タッチの押下は `touchstart` だけから積む。
`pointerup` は iOS でキャンバスにフォーカスを移す処理を含むため変更しない。
離した通知が 2 回届いても、2 回目はクリックにならず (`click: None`)、アプリ側でも押下中の操作は 1 回目で終わっているため影響しない。

## 上流への提案と削除の方針

- 上流の egui (emilk/egui) に同等の `WebOptions` 設定とタッチの扱いの修正を提案する。
- 上流に取り込まれた版へ更新するときは、このディレクトリと `[patch.crates-io]` を削除し、上流側の設定へ移行する。
- それまでに eframe を更新する場合は、新しい版の crate を展開し直して上記の変更をすべて再適用する。
  `git log -p -- apps/web/vendor/eframe` で無改変の取り込みとパッチのコミットを分けてある。
