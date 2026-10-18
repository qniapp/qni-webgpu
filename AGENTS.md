# AGENTS.md

## 最初に読む

READ ../agent-kit/AGENTS.MD BEFORE ANYTHING.

## 最重要ルール: GPU-only

- 量子状態のシミュレーションは **GPU (WebGPU compute shader) でのみ** 行う。CPU 側で state vector / 密度行列 / 測定確率 / ブロッホベクトルなどを計算してはならない。「とりあえず CPU で動かしておく」「fallback として残す」も禁止。GPU で書きにくい場合は GPU シェーダを直すか、ユーザーに止めて相談する。
- **本番経路で GPU → CPU リードバックを発生させてはならない**。ブロッホベクトル / 測定結果 / 状態ベクトルなど GPU 上の値は、視覚化も含めてすべて GPU シェーダ内で完結させる (storage バッファをレンダーシェーダで直接 sample する `STATE_RENDER_SHADER` パターン)。`map_async` であっても per-frame の staging buffer 割り当て / IPC coordination / 1-frame 遅延が発生するため避ける。テストハーネス向けの on-demand readback (`read_state_vector` のように JS から async で呼ばれるもの) は例外として OK。

## 実描画確認

- 実際の描画を確認できる環境をセットアップする。
- 何か実装したときは、作業完了前に必ず `apps/web` の開発サーバを (再) 起動し、ユーザーが通常のブラウザで `http://127.0.0.1:4174/` を開いて確認できる状態にする。
- 外部 GPU 実行の Qiskit 連携またはその API 経路に関わる実装を変更したときは、`apps/qiskit-backend` のバックエンドサーバも (再) 起動し、`http://127.0.0.1:4184/health` と Web UI から動作確認できる状態にする。
- エージェントで Web アプリの見た目を確認するときは、アプリ内ブラウザだけに頼らず `docs/web.md` の agent browser workflow に従い、通常の外部 Chrome / Playwright で実描画を確認する。
- **デザインを修正したあとは必ず Playwright で実際の見た目を確認する**。対象は `apps/web` の UI だけでなく、`docs/design-system/*.html` の仕様ページや mock も含む。lint (`scripts/lint-docs.sh`) や型チェックは「コードの正しさ」を見るだけで「デザインの正しさ」は検証しないため、ルーラ / 仕様ページのプレビューサムネ / mock の各 state バリアント / コンポーネントを使う各ページのレンダリングを Playwright (`mcp__playwright__browser_navigate` + `browser_take_screenshot`) で実際に開いて目視し、意図したとおりに描画されているか確認する。`file://` は MCP からブロックされるので `python3 -m http.server` などで HTTP サーバを立てて `http://127.0.0.1:<port>/...` 経由でアクセスする。
- 動作確認が終わったスクリーンショットはリポジトリに残さず、確認後すぐに `gio trash` で削除する。
- 正しい描画が確認できるまでデバッグを継続する。

## 開発フロー

- 開発はテスト駆動で進める。
- 何かひとまとまりの作業を終えたら、コミットの有無にかかわらず、必ず現在の差分へ並列レビューをかける。コードだけでなく `*.html` / `*.md` などドキュメントの更新も対象とする。観点はコードとドキュメントで切り替える:
  - **コード変更**: correctness（正しさ）/ test coverage（テスト網羅性）/ unnecessary complexity（不要な複雑さ）
  - **ドキュメント変更**: correctness（記述と実装の整合、リンク切れ、構文崩れ、用語の意味的誤り）/ clarity（想定読者への読みやすさ、見出し階層と情報構造の整合、不明瞭な指示語や未定義略語、**ルー語 = 日本語文中の不自然な英日混在の混入**）/ unnecessary complexity（冗長表現、重複説明、過剰訳、空白ノイズ、本筋と無関係な情報）
- 所見は重大度別に要約し、適用価値のある修正だけを反映する。コミットする場合はレビュー対応後に行う。
- `*.html` / `*.md` / `AGENTS.md` を編集したら、コミット前に必ず `scripts/lint-docs.sh` を実行して通過させる。これは用語ゆれ・ルー語 (textlint + prh、用語集は `tools/docs-lint/prh.yml`) / HTML 構造 (html-validate) / Markdown スタイル (markdownlint-cli2) の 3 つを一括で走らせる。新しい用語の方針が決まったら `prh.yml` にも追記し、検出ルールとして残す。
- コミットしたら必ず直後に `git push` し、ローカルだけにコミットを残さない。
- このプロジェクトでは後方互換レイヤーや旧名 wrapper は残さない。名前変更時は旧入口を削除し、新しい名前へ移行する。
- 変更フローは「変更 → テスト → pending snapshots 確認 → review → accept/reject → CI 検証」を遵守する。

## テスト規約

- テストケースごとのアサーションは 1 つまでにする。Playwright / Node / Rust / TUI では `expect` / `assert*` / `assert_*` を 1 test に 1 回だけ置き、複数観点を確認したい場合はテストケースを分割する。setup / helper は失敗時に例外を投げてもよいが、複数アサーションを隠す wrapper にしない。Cucumber は 1 Scenario につき `Then` を 1 つだけにし、`Then` 後の `And` で追加検証しない。
- UI 変更はスナップショットテストで管理し、`cargo insta review` で差分を確認して accept/reject を明示する。
- スナップショットのシリアライズ形式は統一し、改行や CRLF を正規化して差分ノイズを抑える。
- CI で `cargo insta pending-snapshots` を実行し、未承認スナップショットがある場合は失敗させる。

## UI / スタイルガイド

- UI の色・スタイルはガイドラインで統一し、ANSI 標準色を優先してテーマ互換性を保つ。
- **配色は Flexoki Light に厳密に従う** ([kepano/flexoki](https://github.com/kepano/flexoki))。Rust の新規 / 変更色は `apps/web/src/colors/flexoki.rs` の名前付き定数 (`BG` / `BLUE_600` など) を使い、描画側ではその定数で定義した `Colors` の役割別の色を参照する。本番コードではこのパレットモジュールの外に生の RGB 値を書かず、トーンとの対応を示すコメントも書かない。Tailwind 等の独自色 (sky-500 / zinc-600 / emerald-500 など) を直接書かない。
- **タイポグラフィ / スペーシングは Tailwind のデフォルトスケールに揃える** ([tailwindcss.com/docs/font-size](https://tailwindcss.com/docs/font-size) / [tailwindcss.com/docs/customizing-spacing](https://tailwindcss.com/docs/customizing-spacing))。フォントサイズは `text-xs` (12px) / `text-sm` (14px) / `text-base` (16px) / `text-lg` (18px) / `text-xl` (20px) などスケール内のサイズのみ使う。13px / 11px / 9px のようなスケール外サイズは使わない。同様にスペーシング (padding / gap / margin / row pitch) も Tailwind の 4px 倍数スケール (`spacing-1` = 4 / `spacing-2` = 8 / `spacing-3` = 12 / `spacing-4` = 16 / `spacing-5` = 20 / `spacing-6` = 24 / ...) に揃え、`px-3.5` などスケール外の値を避ける。line-height は対応するフォントサイズの Tailwind 既定 (text-sm → 20px / text-xs → 16px) を使う。サイズの指定には `apps/web/src/tailwind.rs` の名前付き定数を使い、Tailwind トークンとの対応を示すコメントは書かない。
- スタイルガイド違反は clippy の禁止 API（disallowed-methods など）で検知する。

## ログ / デバッグ

- デバッグ出力は `println!` を避け、ファイルロギング（tracing など）を使う。

## ドキュメントと言語

- ドキュメントは随時更新する。
- 必要なドキュメントは適宜 docs/*.md に追加する。
- 仕様書、計画書、Linear/GitHub への書き込み、PR レビュー、作業ログ、`docs/*.md` / `docs/*.html` などのドキュメントは日本語で書く。
- 例外として、リポジトリ直下の `README.md` は英語で書き、その日本語版を `README.ja.md` とする。
- 日本語文の中では、不自然な英日混在の「ルー語」を避ける。
- 英語の普通名詞に自然な日本語訳がある場合は、日本語または定着したカタカナ語を使う。
  - 例: `step definition の意図` ではなく「ステップ定義の意図」
  - 例: `env setup` ではなく「環境変数の準備」
  - 例: `command 実行` ではなく「コマンド実行」
  - 例: `rendering pipeline` ではなく「描画パイプライン」
  - 例: `shader を共有` ではなく「シェーダを共有」
  - 例: `fidelity gap` ではなく「見た目の乖離」
  - 例: `wasm-canvas embed` ではなく「wasm-canvas を埋め込む」
- ただし、ツール名、ファイル名、API 名、CLI 引数、環境変数、Gherkin キーワード（`Given` / `When` / `Then`）、コード識別子、固有の技術用語などは原文のまま書いてよい。

## 用語集

ドキュメント内で使う日本語表記とコード識別子の対応をここで一元管理する。新しい用語の方針が決まったら必ずここに追記し、`docs/*` 側もこの表に従って統一する。

- **事後選択** — 指定した量子状態への射影によって、その条件を満たす試行だけを残す操作。コード識別子は `PostSelectOff` / `PostSelectOn` / `PostSelectAntiX` / `PostSelectX`。
- **生存率** — 回路のある位置までのすべての事後選択を通過する累積確率。条件付き状態の正規化とは区別する。
- **検出器** — 指定軸の負の固有状態を標本測定で検出し、検出結果に対応する部分空間へ射影する回路要素。
  - 個別名: `X 軸検出器` / `Y 軸検出器`
  - コード識別子と Quirk 互換のシリアライズ ID: `XDetector` / `YDetector`。

- **ベルペア** — 2 量子ビットのベル状態。コメントブロックの英語ラベルは `Bell pair`。

- **表示ブロック** — 量子状態を可視化するが状態を変えない回路要素の総称 (Quirk が `Display` と呼ぶカテゴリに対応)。ユニタリ変換を行わないため厳密にはゲートではない。
  - 個別名: `ブロッホ球表示ブロック` / `確率表示ブロック` / `振幅表示ブロック`
  - コード識別子: `GateKind::BlochDisplay` / `GateKind::ProbabilityDisplay` / `GateKind::AmplitudeDisplay` (`Display` サフィックス) はそのまま維持。Quirk 互換のシリアライズ ID (`"Bloch"` / `"Probability"` / `"Amps"` ほか) もそのまま。
  - Quirk の `"Chance"` / `"Chance2"`〜`"Chance16"` は `"Probability"` 系の読み込み専用の別名として受理し、書き出しには `"Probability"` / `"ProbabilityN"` を使う。
- **サンプル表示ブロック** — 状態を収縮させずに 1 つのビット列をサンプリングして表示する表示ブロック。
  - コード識別子: `GateKind::SampleDisplay`。シリアライズ ID は `"Sample1"` / `"Sample2"` / `"Sample3"`。
- **パリティ制御** — 同じ列で条件を満たすものが奇数個のときに列の操作を適用する制御。個別名は `Z パリティ制御` / `X パリティ制御`。
  - コード識別子: `GateKind::ZParityControl` / `GateKind::XParityControl`。シリアライズ ID は `"zpar"` / `"xpar"`。
- **縦レイアウト** — 単体アプリで、幅 640 論理 px 未満かつ高さが幅以上のキャンバスに使う画面構成。上に状態ベクトル、中央に縦向きの回路 (時間は上から下、量子ビット 0 が右端)、下にパレットのカルーセルを置く。
  - コード識別子: `CircuitOrientation::Vertical`。対になる従来の構成は **横レイアウト** (`CircuitOrientation::Horizontal`)。
- **パレットのカルーセル** — 縦レイアウトの画面下端に固定する、1 ページ 4 項目のパレット。複数ページあるときは `‹` `›` とページドットでページを切り替える。
  - コード識別子: `PaletteCarousel`。
- **コメントブロック** — 回路の連続した列を囲み、コメントを付ける要素。シミュレーションには影響しない。qni の `circuit-block` に対応する。
  - コメントを枠の上下に表示する文字を **ラベル** と呼ぶ (押すと編集、ドラッグで移動)。縦レイアウトでは左に 1 つ表示する。コード上は `label`。
  - コード識別子: `CircuitBlock` / `CircuitBlocks`。パレットの項目は `PaletteItem::CircuitBlock`。
  - シリアライズ: `"{<コメント>"` / `"}"`。埋め込みの制限パレットでは `"{}"`。
