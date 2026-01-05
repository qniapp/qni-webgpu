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

## リント

```
pnpm lint
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

- 単一量子ビットゲートのみ対応
- 初期状態は |0>
