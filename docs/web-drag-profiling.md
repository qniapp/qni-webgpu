# web Drag Profiling

## Summary (2026-01-23)

- 手法: Playwright + CDP `Profiler.start/stop` でドラッグ中の CPU プロファイルを取得（headless Chromium）。
- 対象: パレットからゲートを配置し、配置済みゲートを上下に高速ドラッグ（5 往復）。

## CPU Profile (Self time, top)

```
CPU profile top (ms):
- (idle)                                   2263.694
- (program)                                174.747
- imports.wbg.__wbg_new_from_slice...        14.673
- epaint::tessellator::fill_closed_path      13.688
- (anonymous)                                9.759
- core::ptr::copy_nonoverlapping...          8.784
- alloc::vec::Vec::extend_from_slice         8.042
- alloc::vec::Vec::append_elements           6.501
- <Iter<T> as Iterator>::all                 6.129
- requestAnimationFrame                      4.662
- core::ptr::const_ptr::offset_from_unsigned 4.652
- Vec<T>::from_iter                          4.449
```

## CPU Profile (Fast drag simplification, Self time, top)

```
CPU profile top (ms):
- (idle)                                   2354.511
- (program)                                153.542
- (anonymous)                               17.133
- imports.wbg.__wbg_new_from_slice...         6.315
- egui::hit_test::hit_test                   5.864
- requestAnimationFrame                      4.918
- (anonymous)                                4.262
- createCommandEncoder                       4.124
- epaint::tessellator::Tessellate...          3.419
- submit                                     3.379
- epaint::tessellator::fill_closed_path       3.273
- bytemuck::bytes_of                          2.584
```

## Observations

- 簡略描画後は `epaint::tessellator` の自己時間が大きく低下。
- `Vec::extend_from_slice` / `copy_nonoverlapping` の比率が減り、**インスタンス生成/一時バッファの負荷が改善**。
- `createCommandEncoder` / `submit` が上位に戻るため、**WebGPU 送出コストは残る**。

## Bottleneck Hypothesis

ドラッグ中の毎フレーム描画で、  

1) WebGPU コマンドの作成/送出、  
2) UI テッセレーション（簡略化後でも残る部分）、  
が主な CPU ホットスポットになっている。

## Notes

- headless Chromium のため SwiftShader 依存。実機 GPU では絶対値が変わる点に注意。
- `idle` が大部分を占めるため、上記は「非 idle の中での相対比較」として扱う。
