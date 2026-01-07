# Ralph - Autonomous AI Coding Loop

Ralph は、寝ている間に機能を実装してくれる自律型 AI コーディングループです。

## 概要

Ralph は AI エージェント（Claude Code）を繰り返し実行し、`prd.json` に定義されたタスクを順次完了させます。各イテレーションは新しいコンテキストウィンドウで実行され、メモリは git コミットとテキストファイルで永続化されます。

## ファイル構成

```
scripts/ralph/
├── ralph.sh       # メインループスクリプト
├── prompt.md      # 各イテレーションの指示
├── prd.json       # タスクリスト（ユーザーストーリー）
├── progress.txt   # 学習ログ（パターン・進捗）
└── README.md      # このファイル
```

## 使い方

### 1. タスクを定義

`prd.json` を編集してユーザーストーリーを追加:

```json
{
  "id": "US-001",
  "title": "Add H gate to palette",
  "acceptanceCriteria": [
    "H gate icon appears in gate palette",
    "Can drag H gate onto circuit",
    "trunk build passes"
  ],
  "priority": 1,
  "passes": false,
  "notes": ""
}
```

**ポイント:**
- `priority`: 小さいほど先に実行
- `passes`: 完了したら `true` に更新される
- タスクは 1 コンテキストウィンドウに収まるサイズに分割

### 2. フィーチャーブランチを作成

```bash
git checkout -b ralph/feature
```

`prd.json` の `branchName` と一致させてください。

### 3. Ralph を実行

```bash
cd apps/egui-web
./scripts/ralph/ralph.sh 10  # 最大10イテレーション
```

### 4. 進捗を確認

```bash
# ストーリーの状態
cat scripts/ralph/prd.json | jq '.userStories[] | {id, title, passes}'

# 学習ログ
cat scripts/ralph/progress.txt

# コミット履歴
git log --oneline -10
```

## 動作フロー

各イテレーションで Ralph は:

1. `prd.json` から優先度の高い未完了タスクを選択
2. 実装
3. チェック実行:
   - `cargo clippy --target wasm32-unknown-unknown`
   - `cargo check --target wasm32-unknown-unknown`
   - `trunk build`
4. パスしたらコミット
5. `prd.json` の `passes` を `true` に更新
6. `progress.txt` に学びを記録
7. 全タスク完了まで繰り返し

## 成功の鍵

### 小さいストーリー

```
❌ 大きすぎ:
  "Build entire quantum gate system"

✅ 適切なサイズ:
  "Add H gate icon to palette"
  "Implement H gate drag-drop"
  "Add H gate tooltip"
```

### 明確な受け入れ基準

```
❌ 曖昧:
  "Users can add gates"

✅ 明確:
  - H gate appears in left palette
  - Can drag H gate to circuit grid
  - Gate snaps to grid position
  - trunk build passes
```

### フィードバックループ

Ralph には高速なフィードバックが必要:
- `cargo clippy` - リントチェック
- `trunk build` - ビルド検証
- `pnpm test` - E2E テスト（UI 変更時）

## 注意事項

- **探索的な作業には不向き**: 明確なゴールがある作業に使用
- **セキュリティクリティカルなコードは避ける**: 人間のレビューが必要
- **1 ストーリー = 1 コミット**: 各ストーリーは独立して完結

## 参考

- [Original Ralph by Geoffrey Huntley](https://ghuntley.com/ralph)
- [Ryan Carson's thread](https://x.com/ryancarson/status/2008548371712135632)
