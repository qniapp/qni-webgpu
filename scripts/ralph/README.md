# Ralph - Autonomous AI Coding Loop

Ralph は、寝ている間に機能を実装してくれる自律型 AI コーディングループです。

## ファイル構成

```
/
├── tasks.md           # タスクリスト
├── progress.md        # 学習ログ
└── scripts/ralph/
    ├── ralph.sh       # メインループ
    ├── prompt.md      # 各イテレーションの指示
    └── README.md      # このファイル
```

## 使い方

```bash
# 1. タスクを編集
vim tasks.md

# 2. フィーチャーブランチを作成
git checkout -b ralph/feature

# 3. Ralph を実行（最大10イテレーション）
./scripts/ralph/ralph.sh 10
```

## タスクの定義

`tasks.md` を編集:

```markdown
## US-001: Add H gate to palette

- [ ] **Status**: pending
- **Priority**: 1

**Acceptance Criteria:**
- H gate icon appears in gate palette
- Can drag H gate onto circuit
- Build passes
```

完了したら `[ ]` を `[x]` に変更。

## 進捗確認

```bash
# 学習ログ
cat progress.md

# コミット履歴
git log --oneline -10
```

## 成功の鍵

- **小さいストーリー**: 1コンテキストウィンドウに収まるサイズ
- **明確な受け入れ基準**: 曖昧さを排除
- **高速フィードバック**: clippy + build が必須

## 参考

- [Original Ralph by Geoffrey Huntley](https://ghuntley.com/ralph)
