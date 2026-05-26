# web Drag Optimization Plan (Quirk 参考)

## 目的

ドラッグ中のゲートがポインタに遅れて追従する問題を解消する。

## Quirk からの方針

参照ファイル:

- `../Quirk/src/ui/sim.js`: 回路が同一ならシミュレーション結果を再利用。
- `../Quirk/src/main.js`: ドラッグ中は preview を更新し、drop 時に commit。
- `../Quirk/src/base/CooldownThrottle.js`: 重い描画ほど間引く。

要点:

- **回路が変わった時だけシミュレーション更新**。
- **ドラッグ中は見た目だけ更新**。
- **重い描画は間引き**。

## 実装方針（qni-webgpu）

### 実装済み

- **GPU 計算計画はドロップ / スナップ時のみ更新**  
  未スナップのドラッグ移動では `needs_recompute` を立てず、回路スロット / 挿入ドロップゾーンへのスナップまたはドロップの時点だけ GPU 計算を走らせる。
- **ドラッグ中の state_count はスナップまで固定**  
  `drag_state_count` で未スナップ中の状態ベクトル長を保ち、スナップ後は仮配置が必要とする状態数まで伸ばす。
- **状態ベクトルのインスタンスをキャッシュ**  
  layout/offset が変わらない限り `StateInstance` を再生成せず、GPU バッファ書き込みを間引く。
- **CooldownThrottle 相当の再描画間引き**  
  Quirk の `REDRAW_COOLDOWN_MILLIS=10` に合わせ、10ms ベース + 0.1 倍のポンプで `request_repaint` と `request_repaint_after` を切り替える。
- **ドラッグ中のカーソル位置を保持**  
  `drag_cursor_pos` を保持してドロップ時の最終位置が欠けないようにする。
- **起動直後の描画ウォームアップ**  
  初期フレームで描画が欠けるケースに備え、短時間だけ再描画を回す。
- **ドラッグ中の簡略描画**  
  tessellator 負荷を下げるため、ドラッグ中は角丸・影・アイコン線・制御線を省略してテキスト描画に切り替える。

### 検討中 / 追加候補

- **プレビュー回路の分離**  
  UI だけの preview を分離し、確定回路の更新を drop のみに厳密化。

## 関連ドキュメント

- プロファイル結果: `docs/web-drag-profiling.md`
