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

## 変更点 3: 高 DPR でのキャンバス寸法

変更したファイル:

- `src/web/events.rs`: `get_display_size` で CSS 寸法、DPR、デバイスピクセル寸法を選択処理へ渡す
- `src/backing_size.rs`: 描画バッファの寸法選択と単体テストを追加
- `src/lib.rs`: `backing_size` モジュールの宣言を追加

### 理由

`src/web/events.rs` の `get_display_size` は、`ResizeObserverEntry.contentRect` の CSS 寸法に `native_pixels_per_point()` を掛けてから丸め、描画バッファの寸法を決める。
`devicePixelContentBoxSize` が存在し、両軸とも CSS 寸法 × DPR を丸めた値との差が 1 デバイスピクセル以内なら、その値を優先する。
実機の小数 DPR でのピクセル境界への整列を保ちつつ、Chrome の DPR エミュレーションで倍率が食い違う値は採用しない。
たとえば CSS 幅 960px、DPR 2 でも `devicePixelContentBoxSize` が幅 960 を返すと、従来の処理は描画バッファを 960px にしていた。
egui はこの幅を DPR 2 で割るため論理幅が 480pt になり、960 CSS px の表示領域では内容が 2 倍に拡大される。

修正後は egui と描画バッファが同じ倍率を使う。
CSS 寸法を先に整数へ丸めず、DPR を掛けた後で丸めるため、小数の CSS 寸法も描画バッファへ反映される。
ホスト側が `canvas.width` / `canvas.height` を CSS 寸法 × DPR に設定する場合と、eframe だけが設定する場合の両方を `tests/embed.spec.ts` で検証する。
チュートリアル側の寸法設定を変更する必要はない。

選択処理は `src/backing_size.rs` に分離した。単体テストは通常の CI の対象外で、ネイティブ環境で次のコマンドを手動実行し、一致、不一致、小数 DPR、値の欠落を検証する。

```sh
cargo test --manifest-path apps/web/vendor/eframe/Cargo.toml \
  --lib backing_size --no-default-features --features glow,x11
```

## 変更点 4: タッチの取消要求をブラウザの条件に合わせる

変更したファイル:

- `src/web/events.rs`: document の `touchmove` を明示的に非 passive として登録し、タッチイベントの `prevent_default()` を `cancelable()` が真の場合だけ呼ぶ。`touchcancel` では追跡中の指を解除し、`PointerGone` と再描画要求を追加する

### 理由

Chromium では document の `touchmove` は、設定を省くと passive として扱われる。
その中で `prevent_default()` を呼ぶと、取消は無視され、ブラウザが介入メッセージを出す。
記事のスクロールが始まった後の `touchend` など、取消不能なイベントでも同様のメッセージが出る。

単体アプリは従来どおり取消を要求できる。埋め込みは `apps/web/src/lib.rs` の設定で、
操作対象の `touchstart` だけを同期的に取り消し、その操作を捕捉する。
後続の移動、解放、ポインタ消失では取消を要求しないため、空白から始めた記事のスクロールを止めない。
移動・終了・取消通知は入力キューに引き続き届け、伝播の設定も維持する。

`touchcancel` は従来、タッチの取消通知しか積んでいなかった。
egui はタッチとマウス相当のポインタを別に管理するため、これだけでは押下状態が残り、次のタップが反応しない。
取消時は追跡を解除して再描画し、qni-webgpu 側でポインタの押下状態と進行中の操作を解除する。
編集中の回路は履歴の作業開始時点に戻す。クリックを発生させる解放イベントは合成しない。
同じフレーム内に新しいタッチがある場合、その押下状態は残す。

`apps/web/tests/embed-mobile-contract.spec.ts` は実際の Chromium のタッチ入力を使い、
記事のスクロール、回路の保持、パン、タップ、ドラッグ、解放後の操作を検証する。
console に加えて CDP Log の介入メッセージも検査する。
これはモバイルの模擬環境での検証であり、実機や Safari での動作を保証するものではない。

## 上流への提案と削除の方針

- 上流の egui (emilk/egui) に同等の `WebOptions` の autofocus 設定、タッチの押下の扱い、高 DPR でのキャンバス寸法、タッチの取消条件の修正を提案する。
- 4 件すべての変更が上流に取り込まれた版へ更新するときは、このディレクトリと `[patch.crates-io]` を削除し、上流側の設定へ移行する。
- それまでに eframe を更新する場合は、新しい版の crate を展開し直して autofocus 設定、タッチの押下、高 DPR でのキャンバス寸法、タッチの取消条件の変更をすべて再適用する。
  `git log -p -- apps/web/vendor/eframe` で無改変の取り込みとパッチのコミットを分けてある。
