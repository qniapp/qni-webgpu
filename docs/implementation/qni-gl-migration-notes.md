# qni-gl から qni-webgpu への移行メモ

ABCI 配備で qni-gl の構成を参照するときに、同じ調査を繰り返さないためのメモ。
qni-webgpu は qni-gl の Docker / nginx / Open OnDemand の考え方を引き継ぐが、API と Web 配信は qni-webgpu 側の制約に合わせて変える。

## 配信方式

| 項目 | qni-gl | qni-webgpu |
| --- | --- | --- |
| Web 配信 | Node / Vite 系の配信。 | Trunk の `dist` を nginx で静的配信する。 |
| Open OnDemand パス | `/node/<host>/<port>/` 配下。 | 同じく `/node/<host>/<port>/` 配下。静的ファイルは相対パスで読み込む。 |
| API パス | `/run` と `/backend.json` を使う。 | `/run` と `/health` を使う。`/backend.json` は使わない。 |
| 認証 | 固定の `.htpasswd` を参照する構成があった。 | 環境変数、パスワードファイル、既存 `.htpasswd` から生成する。 |

## 実行ランナー

qni-webgpu の本番配備では `qiskit-gpu` だけを許可する。
`mock` と `qiskit-cpu-dev` はローカル開発用であり、ABCI 本番の代替経路ではない。

## API 形式

qni-gl はフォーム形式や `requestType=export` を持つ。
qni-webgpu は JSON の `/run` に寄せる。

```json
{
  "qubits": 2,
  "columns": [["H", 1], ["•", "X"]],
  "shots": 1024,
  "outputs": {
    "histogram": true
  }
}
```

レスポンスの量子結果はヒストグラムと表示ブロック単位の結果に限定する。
`status` / `runner` / `qubits` / `shots` / `truncated` などのメタデータは返す。
全状態ベクトルや全確率分布は返さない。

## 測定結果

qni-gl の `measuredBits` は、qni-webgpu の初期 ABCI 対応では採用しない。
測定ゲートを含む場合も、量子結果は `histogram` を主契約とする。
UI が表示ブロックを要求した場合は、対応する有界な表示ブロック結果も返す。

## QASM3 書き出し

qni-gl の `requestType=export` に相当する QASM3 書き出しは、qni-webgpu の `Run GPU` 経路には入れない。
必要になった場合は、実行 API から分けた明示的な書き出し機能として設計する。

## キャッシュ

qni-gl の `CachedQiskitRunner` 相当は、初期 ABCI 対応では使わない。
qni-webgpu の本番サーバはステートレスに保ち、同じ要求でも Qiskit Aer GPU を実行する。
キャッシュを導入する場合は、回路 JSON、出力要求、shots、seed、ランナー、依存ライブラリ版をキーに含める。
