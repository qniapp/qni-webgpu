# 制御条件の形式検証

`ColumnControls::restricted_outside(span_mask)` は範囲内の制御だけを取り除き、範囲外の各ビットの `mask` と `value` を変えないことを検証する。
これが壊れると、QFT や算術ゲートが範囲外の制御を誤って残したり落としたりして、誤った基底状態に作用する。

## 実行方法

リポジトリのルートで、初回のみ Kani 0.68.0 を準備する。
Kani はホスト上で専用の nightly を使い、プロジェクトの stable や wasm32 向けのビルドとは独立して動作する。

```bash
cargo install --locked kani-verifier --version 0.68.0
cargo kani setup
```

Kani の準備に時間がかかるため、当面は CI に追加しない。検証は次の 1 コマンドで行う。

```bash
bash apps/web/scripts/verify-kani-controls.sh
```

実ソースの `VERIFICATION:- SUCCESSFUL` と、一時コピーで除去マスクを反転させた故障版の `VERIFICATION:- FAILED` を確認した場合だけ終了コードが 0 になる。
故障版の失敗は期待どおりの結果であり、実ソースは書き換えない。一時ファイルは終了時に削除する。
`KANI_CONTROLS_LOG_DIR` を指定すると、そのディレクトリに `pass.txt` と `mutant.txt` を保存できる。

## 検証範囲

依存ライブラリのない独立クレート `apps/web/verification/kani-controls` が実際の `controls.rs` と `qubit_bit.rs` を取り込む。
`mask`、`value`、`span_mask` は任意の `u32` とし、前提は `value & !mask == 0` だけである。
連続範囲に限定せず、全 32 ビットを対象に、範囲内の両フィールドの消去と範囲外の両フィールドの保存を 1 つのアサーションで検証する。入力の上限による打ち切りはない。

WGSL、GPU、他の `ColumnControls` メソッド、呼び出し側の範囲生成、wasm32 専用コードは検証しない。
変更時の判断と再検証の手順は、プロジェクト内の [検証スキル](../.agents/skills/verify-kani-controls/SKILL.md) を参照する。
