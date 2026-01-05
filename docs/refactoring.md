# リファクタリング検討ポイント

以下は現状の実装を読み、保守性・拡張性・テスト容易性の観点で分割/整理したい箇所です。

- `apps/web/src/main.ts`: 1ファイルに UI 初期化/量子計算/フォント生成/WebGPU セットアップ/描画ループが混在しているため、責務ごとに分割する（`ui/layout.ts`, `ui/input.ts`, `ui/text.ts`, `gpu/*`, `renderer/renderer.ts` に分離済み）。
- `apps/web/src/main.ts`: `window.__*` のデバッグフラグがグローバルに散らばっているので、デバッグ用の型付き API にまとめる（例: `debug.ts` で `setRenderDone()`/`setFrameDataUrl()` などに集約）。
- `apps/web/src/main.ts`: `vertices` のグローバル可変配列と `addQuad/addRect/addLine/addText` が密結合なので、`VertexBuilder` などのクラス/関数にまとめてバッファ生成を一箇所に寄せる。（部分的に `renderer/renderer.ts` に集約済み）
- `apps/web/src/main.ts`: `shaderCode` を文字列インラインで管理しているため、`*.wgsl` として分離しビルドで読み込む形にすると可読性が上がる。（現状は `gpu/shaders.ts` に分離済み）
- `apps/web/src/main.ts`: フォントのビットマップ定義が巨大なリテラルで埋め込まれているので、`fontGlyphs.ts` に切り出し、用途別にまとまった構造へ整理する。（`ui/text.ts` に分離済み）
- `apps/web/src/main.ts`: WebGPU リソース生成（バッファ/パイプライン/バインドグループ/テクスチャ）が単一関数に集中しているので、生成手順を関数化しテスト可能な最小単位に分割する。（`gpu/init.ts`, `gpu/compute.ts`, `renderer/renderer.ts` に分離済み）
- `apps/web/src/main.ts`: `buildScene` が固定座標と魔法数に依存しているため、設定オブジェクト化してレイアウトの調整点を明示する（例: `layout.ts`）。（`ui/layout.ts` に分離済み）
- `apps/web/src/main.ts`: `applyGateToZero` 内で毎回 `matrices` を生成しているため、定数として外出しして使い回す（副作用のない純粋関数化）。（完了: `domain/complex.ts` に移動済み）
- `apps/web/src/main.ts`: 例外処理と `setStatus` が `init` の中で混在しているので、`Result` 的な戻り値や専用のエラーハンドラ関数に整理する。
- `apps/web/src/main.ts`: テストから参照する `__debugPixel` などが描画ループに強く依存しているため、描画結果の検証用インターフェースを抽象化して Playwright 以外のテストにも使える形にする。

必要になったら以下の小さな単位でユニットテストを足せるようにするのが望ましい。

- `apps/web/src/main.ts`: `formatComplex` と `applyGateToZero` を `domain/complex.ts` などへ移し、数値ロジック単体をテスト可能にする。（完了）
- `apps/web/src/main.ts`: `getGateFromQuery` を `domain/gate.ts` に移し、クエリパースとフォールバックをテスト可能にする。（完了）
