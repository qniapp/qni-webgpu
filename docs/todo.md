# TODO

優先度が高い順に実施する。

1. `apps/web/src/main.ts` からドメインロジックを分離（完了）
   - `formatComplex` と `applyGateToZero` を `domain/complex.ts` へ移動
   - `getGateFromQuery` と `Gate` 型を `domain/gate.ts` へ移動
   - ユニットテスト追加（数値ロジック・クエリパース）

2. `window.__*` デバッグフラグの整理
   - `debug.ts` に集約し、型付きAPIで参照・更新

3. `vertices` と描画プリミティブの分離
   - `VertexBuilder` の導入でバッファ生成を一箇所に集約

4. フォント定義の分離
   - `fontGlyphs.ts` を作成してビットマップ定義を切り出し

5. WebGPUリソース生成の分割
   - パイプライン/バッファ/テクスチャ生成を関数化

6. シェーダを `*.wgsl` に分離
   - ビルド読み込み（Viteのraw import利用）

7. レイアウト/魔法数の設定化
   - `layout.ts` に集約し調整点を明示

## 進捗メモ

- Playwrightでのレンダリング不整合は、図形パイプラインのBindGroupをuniformのみへ修正して解消。
