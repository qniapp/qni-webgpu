# Ralph - Autonomous AI Coding Loop

Ralph は、寝ている間に機能を実装してくれる自律型 AI コーディングループです。

## ファイル構成

```
scripts/ralph/
├── ralph.sh       # メインループ
├── prompt.md      # 各イテレーションの指示
├── prd.json       # タスクリスト
├── progress.md    # 学習ログ
└── README.md      # このファイル
```

## 使い方

```bash
# 1. タスクを編集
vim scripts/ralph/prd.json

# 2. フィーチャーブランチを作成
git checkout -b ralph/feature

# 3. Ralph を実行（最大10イテレーション）
./scripts/ralph/ralph.sh 10
```

## タスクの定義

`prd.json` を編集:

```json
{
  "id": "US-001",
  "title": "Add H gate to palette",
  "acceptanceCriteria": [
    "H gate icon appears in gate palette",
    "Can drag H gate onto circuit",
    "Build passes"
  ],
  "priority": 1,
  "passes": false
}
```

## 進捗確認

```bash
# ストーリーの状態
cat scripts/ralph/prd.json | jq '.userStories[] | {id, title, passes}'

# 学習ログ
cat scripts/ralph/progress.md

# コミット履歴
git log --oneline -10
```

## 成功の鍵

- **小さいストーリー**: 1コンテキストウィンドウに収まるサイズ
- **明確な受け入れ基準**: 曖昧さを排除
- **高速フィードバック**: clippy + build が必須

## 参考

- [Original Ralph by Geoffrey Huntley](https://ghuntley.com/ralph)
