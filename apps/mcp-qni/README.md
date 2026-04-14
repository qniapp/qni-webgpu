# Qni MCP Server

Qni は回路編集と状態ベクトル取得を行う最小構成の MCP サーバです。stdio で動作します。

## 必要環境

- Node.js 18+
- pnpm

## セットアップ

```
pnpm install
```

## 起動

```
pnpm start
```

## Claude Code での登録

プロジェクト内で Qni を有効にする:

```
claude mcp add --scope project --transport stdio qni -- \
  node /home/yasuhito/Work/qni-webgpu/apps/mcp-qni/src/index.js
```

`/mcp` で qni を有効化して使う。

## リント

```
pnpm lint
```

## フォーマット

```
pnpm format
```

## チェックまとめ

```
pnpm check
```

## ツール

- `qni_set_qubits` `{ qubits: number }`
- `qni_place_gate` `{ gate: "X"|"H"|"Y"|"Z"|"S"|"T", target: number, column: number }`
- `qni_clear_circuit` `{}`
- `qni_get_circuit` `{}`
- `qni_run` `{}`

## 返却フォーマット

`qni_run` は JSON 文字列を返します:

```
{
  "qubits": 1,
  "stateVector": [
    [1, 0],
    [0, 0]
  ]
}
```

## 注意

- 複数量子ビット回路は作れるが、対応ゲートは単一量子ビットの `X/H/Y/Z/S/T` のみ
- CNOT / SWAP などの多量子ビットゲートには未対応
- 初期状態はデフォルトでは `|0>`。`qni_set_qubits` 後は `|00...0>` から開始する
