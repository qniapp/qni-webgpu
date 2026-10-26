# Ralph - Autonomous AI Coding Loop

Ralph は、寝ている間に機能を実装してくれる自律型 AI コーディングループです。

## ファイル構成

```
/
├── tasks.md
├── progress.md
└── scripts/ralph/
    ├── ralph.sh
    ├── prompt.md
    └── README.md
```

## 使い方

```bash
vim tasks.md

git checkout -b ralph/feature

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
cat progress.md

git log --oneline -10
```

## 成功の鍵

- **小さいストーリー**: 1コンテキストウィンドウに収まるサイズ
- **明確な受け入れ基準**: 曖昧さを排除
- **高速フィードバック**: clippy + build が必須

## 参考

- [Original Ralph by Geoffrey Huntley](https://ghuntley.com/ralph)
