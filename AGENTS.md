READ ../agent-kit/AGENTS.MD BEFORE ANYTHING.

- 実際の描画を確認できる環境をセットアップする。
- エージェントで egui WebGPU の見た目を確認するときは、アプリ内ブラウザだけに頼らず `docs/egui-web.md` の agent browser workflow に従い、通常の外部 Chrome / Playwright で実描画を確認する。
- 正しい描画が確認できるまでデバッグを継続する。
- 開発はテスト駆動で進める。
- ドキュメントは随時更新する。
- 必要なドキュメントは適宜 docs/*.md に追加する。
- このプロジェクトでは後方互換レイヤーや旧名 wrapper は残さない。名前変更時は旧入口を削除し、新しい名前へ移行する。
- UI 変更はスナップショットテストで管理し、`cargo insta review` で差分を確認して accept/reject を明示する。
- 変更フローは「変更 → テスト → pending snapshots 確認 → review → accept/reject → CI 検証」を遵守する。
- スナップショットのシリアライズ形式は統一し、改行や CRLF を正規化して差分ノイズを抑える。
- UI の色・スタイルはガイドラインで統一し、ANSI 標準色を優先してテーマ互換性を保つ。
- スタイルガイド違反は clippy の禁止 API（disallowed-methods など）で検知する。
- デバッグ出力は `println!` を避け、ファイルロギング（tracing など）を使う。
- CI で `cargo insta pending-snapshots` を実行し、未承認スナップショットがある場合は失敗させる。
