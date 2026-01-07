# Tasks

Branch: `ralph/feature`

## US-001: 状態ベクトル円の青色を修正

- [ ] **Status**: pending
- **Priority**: 1
- **Description**: egui-web の状態ベクトル円の青丸の色が濃すぎるので、適切な色に修正する

**Acceptance Criteria:**
- `../qni` の色定義コードを確認し、同じ色を使用する
- egui-web の状態ベクトル円の青色を修正
- `trunk build` が成功する
- Playwright でブラウザ上の見た目を確認する

**Notes:** 色の定義は `../qni` にあるのでそれを参照すること
