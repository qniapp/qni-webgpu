# MCP Qni サーバ

このドキュメントは MCP サーバ「Qni」の使い方と、提供するツールの仕様をまとめる。

## 目的

- Codex / Claude などの MCP クライアントから回路を編集する
- 回路を実行し、状態ベクトルを取得する

## 位置づけ

- サーバは `apps/mcp-qni` に配置する
- 現状は CPU で状態ベクトルを計算する（WebGPU 連携は後続タスク）

## セットアップ

```
cd apps/mcp-qni
pnpm install
```

起動:

```
pnpm start
```

## Claude Code での登録

プロジェクト単位で登録する場合:

```
claude mcp add --scope project --transport stdio qni -- \
  node /home/yasuhito/Work/qni-webgpu/apps/mcp-qni/src/index.js
```

`.mcp.json` を直接編集する場合:

```
{
  "mcpServers": {
    "qni": {
      "type": "stdio",
      "command": "node",
      "args": [
        "/home/yasuhito/Work/qni-webgpu/apps/mcp-qni/src/index.js"
      ],
      "env": {}
    }
  }
}
```

## ツール一覧

- `qni_set_qubits`
  - 入力: `{ "qubits": number }`
  - 回路を初期化し、量子ビット数を設定する
- `qni_place_gate`
  - 入力: `{ "gate": "X"|"H"|"Y"|"Z"|"S"|"T", "target": number, "column": number }`
  - 指定した量子ビットと列に単一ゲートを配置する
- `qni_clear_circuit`
  - 入力: `{}`
  - 回路の全操作を削除する
- `qni_get_circuit`
  - 入力: `{}`
  - 現在の回路定義を取得する
- `qni_run`
  - 入力: `{}`
  - 回路を実行し、状態ベクトルを返す

## 返却フォーマット

`qni_run` の戻り値は JSON 文字列で、以下の形を取る:

```
{
  "qubits": 2,
  "stateVector": [
    [1, 0],
    [0, 0],
    [0, 0],
    [0, 0]
  ]
}
```

- `stateVector` は `[real, imag]` の配列
- インデックスは `|00...0>` から順の基底
- 対象ゲートは単一量子ビットゲートのみ

## 例

```
# 1量子ビットにHゲートを置く
qni_set_qubits { "qubits": 1 }
qni_place_gate { "gate": "H", "target": 0, "column": 0 }
qni_run {}
```

## 制約

- 2量子ビット以上のエンタングル操作には未対応
- WebGPU 表示との同期は今後の拡張で対応予定
