# Tasks

## US-001: 状態ベクトル円の青色を修正

- [x] **Status**: done
- **Priority**: 1
- **Description**: egui-web の状態ベクトル円の青丸の色が濃すぎるので、適切な色に修正する

**Acceptance Criteria:**
- `../qni` の色定義コードを確認し、同じ色を使用する
- egui-web の状態ベクトル円の青色を修正
- `trunk build` が成功する
- Playwright でブラウザ上の見た目を確認する

**Notes:** 色の定義は `../qni` にあるのでそれを参照すること

## US-004: ワイヤラベルと端点の間隔を修正

- [x] **Status**: done
- **Priority**: 1
- **Description**: 各ワイヤのラベル (q0:, q1:) と、ワイヤの端点の間隔が広すぎるので 0.5rem に修正する

**Acceptance Criteria:**
- ワイヤラベル (q0:, q1: など) とワイヤ端点の間隔を 0.5rem に設定
- `trunk build` が成功する
- Playwright でブラウザ上の見た目を確認する
