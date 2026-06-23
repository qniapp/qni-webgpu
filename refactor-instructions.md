# qni-webgpu リファクタリング指示書

> この文書は実装担当モデル向けの完結した指示書である。`/goal refactor-instructions.md に書かれたことを完遂しろ` という形で渡される前提で書かれている。
> 着手前に必ずリポジトリの `AGENTS.md` (CLAUDE.md と同一ファイル) を読み、本書と併せて従うこと。矛盾がある場合は AGENTS.md を優先し、作業を止めて報告する。

## Objective

既存仕様・既存挙動を一切壊さずに、2026-04 以降のリファクタリングで残った技術的負債を減らし、今後の変更を安全にしやすい構造にする。

- 目的は「見た目を綺麗にすること」ではない。各変更は「将来の変更コストが下がる」「壊れたときに気づける」のどちらかに直結していなければならない。
- このリポジトリは過去に大規模なリファクタリング (監査記録: `docs/refactor-candidates.html` の C-01〜C-11) を経ており、**大半の候補は実装済み**である。本書はその残存項目と、今回の調査で新たに確認した負債だけを対象にする。済んでいる項目を蒸し返さないこと。
- 証拠なく大きな削除や全面書き換えをしてはならない。本書の Debt Map に列挙された範囲だけを、フェーズ順に小さく進める。

## Project Understanding

### 何のプロジェクトか

ブラウザ上で動く WebGPU ベースの量子回路エディタ / シミュレータ (Qni の後継)。Rust + egui を WebAssembly にビルドして配信し、状態ベクトル計算と可視化を WebGPU compute / render shader 上で完結させる。

主要なユーザー体験:

1. ゲートパレットから回路へドラッグしてゲートを配置・編集する
2. 編集と同時にローカル WebGPU シミュレーション (最大 16 量子ビット) が走り、状態ベクトル / ブロッホ球 / 確率 / 振幅 / 密度行列の表示ブロックが更新される
3. `Run GPU` で外部 Qiskit バックエンド (最大 32 量子ビット、Qiskit Aer / cuStateVec) に回路を送り、ヒストグラムと表示ブロック結果だけを受け取って表示する
4. 回路は URL (Quirk 互換 JSON) と localStorage の回路ライブラリで保存・共有できる

### リポジトリ構成 (3 つの独立アプリ)

ルートに Cargo ワークスペースは無く、各アプリが独立している。

- `apps/web` — 本体。Rust (wasm32) + egui + wgpu。Trunk でビルド。内部に `crates/circuit-library-model` (サンプル回路定義) と `crates/external-gpu-model` (外部 GPU 実行の値オブジェクトとペイロード生成) のサブ crate を持つ
- `apps/tui` — ratatui 製の独立 PoC。**CPU シミュレータを持つが、GPU-only ルールの対象外** (AGENTS.md と利用者メモで確認済み)。web とコード共有は無い
- `apps/qiskit-backend` — Python 製 HTTP バックエンド。`/health` (GET) と `/run` (POST)。ランナーは `mock` / `qiskit-cpu-dev` / `qiskit-gpu` の 3 種で、本番コンテナは entrypoint (`deploy/docker/docker-entrypoint.sh`) が `qiskit-gpu` 以外を拒否する

### 主要なエントリーポイントとデータフロー

起動: `apps/web/index.html` → `bootstrap.ts` (wasm ロード、テストフック `window.__qni*` / `__egui*`、Qiskit バックエンド呼び出し用 JS) → `src/lib.rs` の `start(canvas_id)` (142 行の thin entry) → `src/app.rs` の `QniApp` → `app/update_flow.rs` がフレームごとの処理順を統括。

ローカルシミュレーションの流れ:

```
PlacedGate (column / wire / span が正本)
  → simulation_plan/linearize.rs : linearize_ops() で SimulationOp 列へ
  → simulation_plan/capacity.rs : validate_simulation_plan_capacity() で容量検証
  → app/gpu_plan_state.rs : GpuPlanState にキャッシュ
  → gpu/recompute/* : compute shader を dispatch、結果は storage バッファに常駐
  → gpu/resources/* + gpu/callbacks/* : render shader が storage バッファを直接 sample
```

本番経路に GPU→CPU リードバックは無い。`gpu/readback.rs` の async readback (`read_state_vector` ほか) は **テストハーネス専用の例外**。

外部 GPU 実行の流れ: `app/external_gpu/app_adapter.rs` が回路を検証し `crates/external-gpu-model` でペイロード生成 → `client.rs` → JS (`bootstrap.ts` の `__qniRunQiskitBackend`) → Python `/run` → ヒストグラム + 表示ブロック結果の JSON → `external_gpu/{amplitude,bloch,probability,density}.rs` がパースして GPU バッファへアップロード。

### 外部依存

- Rust: eframe / egui / egui_wgpu / wgpu / wasm-bindgen (apps/web)、ratatui + vendor の ratatui-testlib (apps/tui)
- JS: pnpm、Playwright、Cucumber、TypeScript (テストと bootstrap のみ。UI フレームワークは無い)
- Python: qiskit==1.2.1、qiskit-aer==0.15.1、numpy==1.26.4 (固定。動かさない)
- 配備: Docker / nginx / Apptainer / Open OnDemand (`deploy/`、ルート `Dockerfile`)

### テスト構成

- Rust 単体テスト: apps/web に約 300 個 (`check-rust.sh` がホストネイティブでも `cargo test --lib --features eframe/x11` で実行)、apps/tui に約 57 個 + insta スナップショット + PTY E2E
- Playwright: `apps/web/tests/` に 23 spec / 約 258 テスト
- Cucumber BDD: `apps/web/features/` に 3 feature / 6 シナリオ (起動・ドラッグ Z-order・非対応ブラウザのエラー)
- Node: `apps/web/test-node/` (UI 定数同期ほか) とルート `test-node/` (配備設定のコンプライアンステスト)
- Python: `apps/qiskit-backend/tests/test_contract.py` (契約・サーバ・ランナー。GPU ランナーは環境が無ければスキップ)
- CI (`.github/workflows/ci.yml`) は 4 ジョブ構成だが、**GitHub Actions の無料枠枯渇で 2026-06 現在停止中**。検証はローカルで完結させること

## Behaviors To Preserve (絶対に壊してはいけない挙動)

1. **GPU-only**: apps/web の本番経路で量子状態を CPU で計算しない。GPU→CPU リードバックを本番経路に追加しない (テスト用 on-demand readback は現状維持)
2. **URL 回路の往復**: Quirk 互換 JSON (`#{"cols":[...]}`) のエンコード / デコード。シリアライズ ID (`"Bloch"` / `"Probability"` / `"Amps"` など) は変更禁止
3. **localStorage の回路ライブラリ形式**: 保存済みユーザーデータがある。`src/circuit_library.rs` / `app/circuit_library.rs` の読み書き形式を変えない
4. **Qiskit バックエンド HTTP 契約**: `/run` / `/health` のリクエスト・レスポンス JSON フィールド、`x-qni-backend-token` ヘッダ、CORS、ランナー許可制御。Python 側 `contract.py` の制限値 (MAX_SHOTS=100000、MAX_QUBITS=32 ほか) と禁止出力 (statevector 系)
5. **wasm エクスポートとテストフック**: `lib.rs` の `start` / `read_state_vector` / `circuit_library_*`、`bootstrap.ts` と `test_hooks.rs` が公開する `window.__qni*` / `__egui*`。Playwright / Cucumber がこれらに依存している
6. **容量エラーメッセージの文言**: `simulation_plan/capacity.rs` のエラーメッセージは `apps/web/scripts/check_capacity_errors.rs` (check-rust.sh が rustc でビルドして実行する独自チェック) の検証対象。文言を変える場合はチェッカーも同時に直し、理由を報告する
7. **UI 定数の Rust→TS 同期**: `src/constants.rs` の定数は `scripts/generate-ui-constants.ts` が `test-support/generated-ui-constants.ts` へ生成する。定数の改名・移動は `pnpm run check:ui-constants` が壊れる。動かす場合は生成スクリプトも追従させる
8. **見た目**: ゲート描画、ドラッグプレビュー、パレット、表示ブロック、状態パネルのピクセル単位の見た目。描画コードに触れたら Playwright スクリーンショットで目視確認する
9. **実行モードの切り替え挙動**: Local⇔Gpu の容量 (16 / 32)、URL パラメータでの exec mode 指定、GPU モード時の表示ブロックプレースホルダ

## Non-Negotiables (作業全体の制約)

- 最初に `git status` を確認する。**未認識の working tree 変更があっても勝手に restore / reset / revert しない** (このリポジトリは複数エージェントが並列作業することがある)。自分の変更と既存の変更を混ぜない
- 編集前にベースラインの検証結果 (Baseline Commands の出力要約) を記録する
- 変更は小さく戻しやすい単位にし、1 論点 = 1 コミットにする。コミットしたら直後に `git push` する (作業ブランチ: `yasuhito/リファクタリング`)
- 無関係な整形・ついでのリファクタリングをしない。`cargo fmt` の差分が変更ファイル外へ広がったら戻す
- 既存挙動を勝手に変えない。「直したほうが良さそうなバグ」を見つけても黙って直さず、Stop And Ask に従う
- 後方互換レイヤーや旧名 wrapper を残さない。名前を変えるなら旧入口を消して参照を全部更新する
- テスト規約: 1 テスト 1 アサーション。Cucumber は 1 Scenario に `Then` 1 つ
- UI / スタイル: 色は Flexoki Light の名前付きトーン、サイズ・間隔は Tailwind スケールのみ。ソースコメントに対応トークンを明記
- ドキュメントは日本語で書き、ルー語 (不自然な英日混在) を避ける。`*.md` / `*.html` を編集したらコミット前に `./scripts/lint-docs.sh` を通す
- デバッグ出力に `println!` を使わない
- ひとまとまりの作業を終えるたびに、差分へ並列レビュー (correctness / test coverage / unnecessary complexity) をかけ、適用価値のある指摘だけ反映してからコミットする
- 作業完了前に `apps/web` の開発サーバを (再) 起動し、`http://127.0.0.1:4174/` で確認できる状態にして終える

## Stop And Ask Conditions (実装を止めて質問する条件)

以下に該当したら、その項目の実装を止めて質問としてまとめ、他の独立した項目に進む。

1. 正しい仕様がコードからもテストからも `docs/` からも判断できない
2. テストと実装が矛盾している
3. 削除候補 (dead_code 等) が本当に不要か判断できない — 特に `docs/implementation/*.html` の値オブジェクト仕様ページに載っているメソッドは「仕様上の API」かもしれない。仕様ページを確認し、載っていれば残す。判断が割れたら質問する
4. 公開 API (wasm エクスポート、HTTP 契約、URL 形式)、localStorage 保存データに影響しうる
5. 認証・外部連携 (Qiskit バックエンド、nginx、配備設定) に影響しうる
6. 互換性を壊す可能性がある
7. 複数の設計案がありプロダクト判断が必要
8. 作業中に、本書がカバーしていない新たな設計判断 (仕様の分岐、互換性のトレードオフなど) が必要になった (末尾の Q-1〜Q-4 は回答済みだが、別の同種の判断が出てきた場合)

## Baseline Commands (検証コマンド)

実装開始前に以下を実行し、結果 (成功 / 失敗と所要時間) を記録する。失敗するものがあれば、それは自分の変更によるものではないので、その旨を記録して報告する (直そうとしない)。

```bash
# 前提 (初回のみ)
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
pnpm -C apps/web install
pnpm -C apps/web exec playwright install chromium

# 軽量チェック (変更のたびに回す)
cd apps/web && cargo check --target wasm32-unknown-unknown
cd apps/web && cargo clippy --locked --target wasm32-unknown-unknown --tests -- -D warnings
cd apps/web && RUSTFLAGS="-Awarnings" cargo test --locked --lib --features eframe/x11
pnpm -C apps/web run test:preflight

# Rust 一括 (fmt / clippy / ネイティブ unit test / capacity チェッカー / insta)
apps/web/scripts/check-rust.sh

# E2E (UI / GPU 挙動に触れたら)
pnpm -C apps/web run test:bdd
xvfb-run -a -s "-screen 0 1920x1080x24" pnpm -C apps/web exec playwright test

# TUI に触れたら
make check          # ルート Makefile → apps/tui の fmt/clippy/test/insta/audit/deny

# Python バックエンドに触れたら (python3.11 venv 推奨)
cd apps/qiskit-backend && python3 -m venv .venv && .venv/bin/pip install -e . \
  && .venv/bin/python -m unittest discover -s tests

# ドキュメントに触れたら
./scripts/lint-docs.sh

# 仕上げ (全部)
./scripts/check-all.sh

# 実描画確認用の開発サーバ
cd apps/web && trunk serve --address 127.0.0.1 --port 4174 --no-autoreload
```

注意: trunk は Rust の変更を検知しないことがある (キャッシュ不整合の既知の落とし穴)。描画確認で変更が反映されない場合は trunk のプロセスを再起動する。ポート 4184 は別チェックアウトの Qiskit バックエンドが使っていることがあるので、kill する前に必ず該当プロセスの cwd を確認する。

## Debt Map (負債一覧)

> 優先度: A = 今回実装してよい / B = 質問の回答後に実装 / C = 提案に留める (承認なしに実装しない)

### D-01. `PlacedGate.pos` の二重表現 — 優先度 A (カプセル化のみ) + C (除去)

- **根拠**: `apps/web/src/app/circuit_model.rs:25-88`。`column` / `wire` / `span` が正本と明記されている一方、派生値 `pos: egui::Pos2` がフィールドとして常駐し、`sync_pos_from_grid()` (呼び出し 7 箇所) で手動同期している。`.pos` の読み出しは約 33 箇所
- **なぜ負債か**: 正本と派生値が同じ構造体に同居し、同期漏れがあると「モデルは正しいのに描画やヒットテストだけずれる」型の不具合になる。コンパイラはこの不変条件を守ってくれない
- **影響範囲**: render/ 各所、drag_controller、span_resize、layout
- **変更リスク**: ドラッグ中だけ `pos` がポインタ追従の一時値になる仕様があるため、機械的な除去は挙動を壊す
- **改善案 (実装してよい範囲)**: フィールドへの直接書き込みを止め、`circuit_model` 内に閉じる。具体的には (a) `pos` への代入箇所を洗い出し、`sync_pos_from_grid` か、ドラッグプレビュー専用の書き込みメソッド (例: `fn set_drag_preview_pos(&mut self, pos: egui::Pos2)`) 経由に統一する、(b) doc コメントに不変条件 (配置確定済みゲートでは pos は常に `grid_pos(column, wire)` と一致) を明記する
- **改善案 (提案に留める範囲)**: `pos` をフィールドから除去して描画時に `layout` から計算する設計変更。差分が広範囲に及ぶため、設計案と影響箇所一覧を提示して承認を得てから
- **検証**: `cargo test --lib` + Playwright のドラッグ系 spec (`web-drag-visual` / `web-interaction`) + 実描画でドラッグプレビュー確認

### D-02. `QniApp` のフィールド平置き (神オブジェクトの残り) — 優先度 A

- **根拠**: `apps/web/src/app.rs:44-138`。56 フィールド。既に `PickerState` / `StatePanelState` / `GpuPlanState` / `DragState` への抽出は済んでいるが、以下が平置きのまま:
  - 外部 GPU 関連 13 個 (`external_gpu_status` / `external_gpu_started_at` / `external_gpu_state_refresh_pending` / 4 つの `*_uploads` / `external_gpu_display_generation` / 4 つの `pending_external_*_slots` / `pending_external_gpu_run_id`)
  - picker 関連の平置き 5 個 (`picker: PickerState` 本体は除く。`picker_hover_suppressed_at` / `picker_drag_suppressed_until_release` / `picker_submenu_toggle_suppressed_until_release` / `picker_drag_animation_epoch` / `picker_overlay_rect`)
  - ホバー状態 7 個 (`hovered_gate_id` / `hovered_step` / `hovered_probability_outcome` ほか)
  - FPS HUD 4 個 (`fps_hud_visible` + 3 つの履歴)
  - ドラッグ補助 (`drag_cursor_pos` / `drag_repaint_deadline` / `drag_repaint_pending` / `pointer_was_down` ほか)
- **なぜ負債か**: 関連状態のライフサイクル (リセットタイミング・整合性) が構造体の形に表れず、フィールド追加のたびに `new()` と各所の手作業同期が増える
- **影響範囲**: app/ 配下と render/ 配下のフィールドアクセス全般
- **変更リスク**: borrow checker の都合で `&mut self` の分割借用が必要になり、機械的な置換では済まない箇所が出る。1 グループずつ進めること
- **改善案**: グループごとにサブ構造体へ移す。順序は (1) FPS HUD → `FpsHud` (最小・参照箇所が少ない)、(2) 外部 GPU 13 個 → `ExternalGpuState` (external_gpu/ モジュール内に置く)、(3) picker 関連 5 個 → 既存 `PickerState` へ統合、(4) ホバー 7 個 → `HoverState`。各グループを独立コミットにし、純粋な移動 (ロジック変更なし) に徹する
- **検証**: グループごとに `check-rust.sh` + `test:bdd` + Playwright。外部 GPU グループは `web-external-gpu-amplitude.spec.ts` を必ず通す
- **補足**: `app.rs:33` の `#[allow(unused_imports)]` 付き re-export ブロックはこの作業の中で精査し、実際に使われている再エクスポートだけ残して allow を外す

### D-03. 表示ブロックのスロット解決 4 連コピーと Bloch だけの非対称 — 優先度 A (Q-1 回答済み: バグ確定)

- **根拠**: `apps/web/src/render/circuit_gates.rs:60-113`。`amplitude_display_slot` / `probability_display_slot` / `density_display_slot` は「GPU モード (placeholders active) なら外部スロットのみ、それ以外は外部→ローカルの順でフォールバック」という同一構造。**`bloch_display_slot` (76 行目) だけ placeholders_active の分岐が無く、GPU モードでもローカル `gpu_plan` スロットへフォールバックする**。説明コメントは無い
- **なぜ負債か**: 同じ概念の処理が 4 回書かれ、しかも 1 つだけ黙って挙動が違う。**Q-1 で「バグ」と確定済み**。GPU 実行モード (`ExecMode::Gpu`) で外部結果がまだ無いとき、振幅・確率・密度は空 (プレースホルダ) を出すのに、ブロッホ表示だけローカル WebGPU の (しばしば古い) 計算結果へフォールバックして表示してしまう。表示種ごとに挙動がバラつく不整合。なお、ここで言う「ローカル」もローカル WebGPU 計算であり CPU ではない (GPU-only ルール違反ではない)。問題は「外部 GPU 実行モードなのに外部でなくローカル GPU の結果を見せる」表示の一貫性の欠如
- **影響範囲**: GPU モードでの表示ブロック描画 (とくに Local→GPU 切替直後や外部実行前の状態)
- **変更リスク**: 統一すると GPU モードのブロッホ表示挙動が変わる (これは意図した修正)
- **改善案 (バグ修正として実装する)**: (1) 先に 4 つのスロット解決の現挙動を固定する単体テストを書く (Phase 1)、(2) `bloch_display_slot` を他 3 種と同じく「GPU モード (`external_gpu_display_placeholders_active()`) では外部スロットのみ、それ以外は外部→ローカルの順」の分岐に揃える、(3) Phase 1 で書いたブロッホのテスト期待値を新挙動に更新する、(4) その上で表示種別を引数に取る共通ヘルパへ 4 つを集約する
- **検証**: 新規単体テスト + `web-bloch-display.spec.ts` + `web-external-gpu-amplitude.spec.ts` + GPU モードの実描画確認

### D-04. gpu/resources の表示系 3 ファイルが分割パターン未適用 — 優先度 A

- **根拠**: `gpu/resources/probability_display.rs` (778 行) / `amplitude_display.rs` (694 行) / `density_matrix_display.rs` (427 行) は compute pipeline と render pipeline が 1 ファイルに同居。一方、同種の `bloch_display/` (reduce / overlay / pipeline)、`state/` (compute / render / pipeline)、`measure/` (reduce / collapse / buffers) は既にサブモジュール分割済みで、リポジトリ内に確立されたパターンがある
- **なぜ負債か**: 同じ層で構成規則が割れており、巨大ファイル側は差分レビューの範囲が無駄に広がる
- **影響範囲**: gpu/resources 内部のみ (公開シグネチャは変えない)
- **変更リスク**: 低。純粋なコード移動に徹すれば挙動は変わらない。ただし `gpu/resources/mod.rs:123` 付近 (`update_target_format`) のコメントにある通り、surface format 変更時に**全表示種の描画パイプラインを再構築する**必要があるという既知の落とし穴がある。分割後も再構築対象の列挙から 3 表示種を漏らさないこと
- **改善案**: 既存の `bloch_display/` の分割粒度に揃えて 3 ファイルをサブモジュール化する。ロジックの変更・「ついでの改善」は一切しない
- **検証**: `check-rust.sh` + 各表示ブロックの Playwright spec (`web-probability-display` / `web-amplitude-display` / `web-density-display`) + 実描画確認

### D-05. `render/circuit_gates.rs` (1191 行) の責務混在 — 優先度 A

- **根拠**: egui painter による描画と、GPU overlay callback (instance データ) の組み立てが 1 ファイルに同居。リポジトリには `render/circuit_connectors/` などの分割前例がある
- **なぜ負債か**: ゲートの見た目を直すだけの変更でも GPU instance 組み立てコードと同じファイルを触ることになる
- **影響範囲**: render/ 内部のみ
- **変更リスク**: 中。描画順序 (painter への発行順) を変えると Z-order が壊れる。move だけ行い、呼び出し順は保持する
- **改善案**: `render/circuit_gates/` 配下へ「ゲート本体描画」「GPU callback 組み立て」「ホバーポップアップ」程度の粒度でサブモジュール化。D-03 のスロット解決ヘルパ集約と同時にやらない (コミットを分ける)
- **検証**: `check-rust.sh` + `test:bdd` (drag-preview-z-order シナリオが Z-order を守る) + Playwright 表示系 spec + 実描画確認

### D-06. Rust↔Python の外部 GPU 契約が暗黙の二重定義 — 優先度 A (文書と検証) / C (自動生成)

- **根拠**: 制限値が `apps/qiskit-backend/src/qni_qiskit_backend/contract.py` (MAX_SHOTS=100_000, MAX_QUBITS=32, MAX_AMPLITUDE_OUTPUTS=32 ほか) と `apps/web/crates/external-gpu-model/src/lib.rs` (`Shots::MAX = 100_000`、コメントで「バックエンドの MAX_SHOTS」と言及) に分かれて存在。JSON フィールド形状は Rust 側に明示的スキーマが無く、`external_gpu/*.rs` のパース処理が事実上の契約。ビット順 (q0=LSB) の規約は Python 側コメント (`runners.py`) にしか書かれていない
- **なぜ負債か**: 片側だけ変更したときに気づく仕組みが「結合テストがたまたま落ちる」しかない
- **影響範囲**: 外部 GPU 実行経路全体
- **変更リスク**: 文書化と検証追加だけなら低
- **改善案 (実装してよい範囲)**: (a) `docs/implementation/external-gpu-api-compatibility.md` に契約の正本表 (エンドポイント、フィールド、制限値、ビット順規約、両側の定義場所) を追記または整備する。(b) `crates/external-gpu-model` に「Rust 側定数 = 文書の値」を固定する単体テストを置き、コメントで Python 側の定義場所をファイルパスつきで相互参照する (例: `// apps/qiskit-backend/src/qni_qiskit_backend/contract.py の MAX_SHOTS と同期`)
- **改善案 (提案に留める範囲)**: スキーマの一本化 (JSON Schema / OpenAPI / コード生成)。ビルドパイプラインへの影響が大きいため提案のみ (Q-3)
- **検証**: `./scripts/lint-docs.sh` + external-gpu-model の `cargo test` + Python `unittest`

### D-07. Python `contract.py` の control_mask / control_value 検証 4 連コピー — 優先度 A

- **根拠**: `contract.py` 内で amplitude / bloch / probability / density の各出力リクエスト検証に同型の control_mask / control_value チェックが繰り返されている (250-254 / 297-301 / 354-358 / 411-415 行)
- **なぜ負債か**: 検証ルールを直すとき 4 箇所の同期が必要
- **影響範囲**: `/run` のリクエスト検証のみ
- **変更リスク**: 低。`tests/test_contract.py` に検証ケースが揃っている
- **改善案**: 共通ヘルパ関数へ抽出する。エラーメッセージの文言は変えない (テストが文言依存の可能性があるため、変える場合はテストと併せて)
- **検証**: `python -m unittest discover -s tests` 全通過

### D-08. `test` と `test:pw-legacy` が同一内容の重複スクリプト — 優先度 C (Q-4 回答済み: 今回は保留)

- **根拠**: `apps/web/package.json` で両方とも `playwright test`。CI (`.github/workflows/ci.yml`) と `scripts/check-all.sh`、`WORKFLOW.md` は `test:pw-legacy` を参照
- **なぜ負債か**: 「legacy」という名前が実態 (現役の Playwright スイート) と乖離しており、新規参加者を誤誘導する。旧名 wrapper を残さない方針にも反する
- **影響範囲**: package.json、ci.yml、check-all.sh、WORKFLOW.md、docs/web.md などの参照箇所、外部のオーケストレーション設定
- **変更リスク**: リポジトリ外の自動化 (Symphony / Linear 連携) が `test:pw-legacy` を直接叩いている可能性がある
- **改善案 (Q-4 で「今回は保留」と確定)**: 今回のリファクタリングでは**触らない**。理由は、`test:pw-legacy` をリポジトリ外の自動化 (Symphony / Linear 連携) が参照しており、実装担当モデルがその外部参照を安全に更新できないため。同一内容なので動作上の実害は無く、負債としては軽微。将来、運用側で外部自動化の参照を更新できる目処が立った時点で `test` への一本化に格上げする (その際はリポジトリ内の全参照を同一コミットで更新する)
- **検証**: 今回は変更しないため検証なし

### D-09. `#[allow(dead_code)]` の点在 — 優先度 A (条件付き)

- **根拠**: `app/circuit_model/wire_index.rs:14,28`、`app/circuit_model/column_index.rs:7,21`、`gates/angle.rs:65,70`、`crates/circuit-library-model/src/lib.rs:454`
- **なぜ負債か**: 本当に死んでいるのか、値オブジェクトとして意図的に揃えた API なのか判別できず、リファクタ時の「消してよいか」判断を毎回発生させる
- **影響範囲**: 各値オブジェクトモジュール
- **変更リスク**: 低〜中。`docs/implementation/` に値オブジェクトの仕様ページ (circuit-column-index.html / wire-index.html など) があり、仕様上の API かもしれない
- **改善案**: 各 allow について (1) 使用箇所を grep、(2) 対応する仕様ページを確認 (例: `wire_index.rs` なら `docs/implementation/wire-index.html` を開き、当該メソッドが API として記載されているか見る)。仕様に載っていれば allow を残し「仕様ページ参照」のコメントを付ける。どこにも根拠が無ければ削除する。判断が割れたら Stop And Ask
- **検証**: `check-rust.sh` (clippy -D warnings が allow 漏れを検出する)

### D-10. Playwright テストのスリープ依存 — 優先度 A (限定的に)

- **根拠**: `tests/web-aspect-dropdown.spec.ts` の `waitForTimeout(50)` / `waitForTimeout(120)`、`tests/web-circuit-picker-reorder.spec.ts` の `waitForTimeout(200)` / `waitForTimeout(80)` と手書きポーリング `waitForCondition()`
- **なぜ負債か**: 実行環境の速度差でフレーキーになる
- **影響範囲**: 当該 spec のみ
- **変更リスク**: 中。アニメーション完了などの確定的なシグナルが無い箇所を無理に書き換えると、かえって不安定になる
- **改善案**: 確定的な条件で待てる箇所だけ `expect.poll` / `waitForFunction` に置き換える。「確定的な条件」とは、`window.__qni*` テストフック経由で読める状態変化や DOM / canvas 属性の変化など、完了をコードで判定できるもの。アニメーション経過時間のように確定シグナルが無い箇所は触らず、コメントで理由を残す
- **検証**: 当該 spec を 3 回連続で実行して全て通ること

### D-11. `gpu/readback.rs` の並行ハンドル構造体 5 連コピー — 優先度 C

- **根拠**: `gpu/readback.rs:24-68` に Bloch / Measurement / Probability / Amplitude / Density の GpuHandle 構造体が同型で並ぶ (509 行、テスト専用経路)
- **なぜ負債か**: 機械的な重複だが、テスト専用でホットパスではない
- **改善案**: ジェネリクスかマクロでの統一を提案のみ。テストハーネスの安定性 > 抽象化の綺麗さ、という判断があり得るため承認を得てから
- **検証 (実装する場合)**: readback を使う Playwright spec 全通過

### D-12. TUI の巨大ファイルと web とのゲート定義重複 — 優先度 A (Q-2 回答済み: 凍結・文書化のみ)

- **根拠**: `apps/tui/src/render.rs` (1104 行) / `lib.rs` (1020 行) / `model.rs` (781 行)。基本ゲート (X/H/Y/Z/S/T/Rx/Ry/Rz など) の定義が web 側 `gates.rs` と二重に存在する。ただし TUI は意図的に独立した CPU PoC であり、docs/decisions.md にも精度選択 (f64 vs f32) の設計根拠が記録されている
- **なぜ負債か**: ファイル肥大は保守コストだが、重複自体は「独立 PoC」という設計判断の帰結であり、悪と決めつけない
- **改善案 (Q-2 で「凍結・文書化のみ」と確定)**: `docs/tui.md` か `docs/design.md` に「TUI は CPU シミュレーションを許容する独立 PoC であり web の GPU-only ルール対象外。web とコードを共有しない (ゲート定義の重複は独立 PoC ゆえの意図的な帰結)」という境界を明文化するだけに留める。**TUI のコード (ファイル分割・重複解消) には今回は手を入れない**。将来分割が必要になったら別タスクとして切り出す
- **検証**: `./scripts/lint-docs.sh`

### D-13. 文書の鮮度ずれ (軽微) — 優先度 A

- **根拠**: `progress.md` が `lib.rs` の 2117 行目・22 行目を参照しているが、現在の `lib.rs` は 142 行 (過去の Ralph ループの遺物。`tasks.md` は自己申告で歴史的記録)。`docs/refactor-candidates.html` の進捗注記は 2026-04 時点で止まっている
- **なぜ負債か**: 新規参加者 (および後続のエージェント) が古い行番号や「未着手」表記を信じて誤誘導される
- **改善案**: `progress.md` の冒頭に `tasks.md` と同様の「これは歴史的記録」断り書きを 1 行足す。`docs/refactor-candidates.html` は本リファクタリングのフェーズ完了ごとに該当候補の進捗注記を更新する。大規模な書き直しはしない
- **検証**: `./scripts/lint-docs.sh`

### D-14. Python バックエンドの例外体系とログの不統一 — 優先度 C

- **根拠**: `server.py` は `CircuitBuildError` / `ContractError` / `RunnerUnavailable` + 汎用 `Exception` を個別捕捉。ログは `logging` モジュールではなく `QNI_QISKIT_BACKEND_LOG` 環境変数ゲートの独自実装
- **なぜ負債か**: 例外追加時の取りこぼしリスクと、本番でのログレベル制御不能
- **改善案**: 基底例外クラス導入と `logging` への移行を提案のみ。現状はテストが充実しており動いているため、配備 (Docker / ABCI) への影響確認を含めて承認を得てから
- **検証 (実装する場合)**: Python unittest + ルート `test-node/` の配備テスト + Docker スモーク

## Implementation Phases (実装フェーズ)

各フェーズの完了条件は「そのフェーズの検証コマンドが全て通り、差分への並列レビューを反映し、コミット + push 済みであること」。フェーズ内の項目も 1 コミット単位で進める。

### Phase 0 — 現状確認 (変更なし)

1. `git status` / `git branch` を確認し、ブランチ `yasuhito/リファクタリング` 上でクリーンであることを記録する。未認識の変更があれば触らずに報告する
2. Baseline Commands の軽量チェック〜Rust 一括までを実行し、結果を記録する (E2E と check-all.sh は時間があれば。少なくとも `test:bdd` は流す)
3. 末尾の「実装前に確認すべき質問」(Q-1〜Q-4) は全件回答済みで各項目に反映済み。改めて質問する必要はない。作業中に本書がカバーしていない判断が必要になったら Stop And Ask に従う

### Phase 1 — 安全網の追加 (挙動変更なし)

このフェーズの目的は「現在の挙動を一切変えずに、テストで挙動を確定させ、後続フェーズの回帰を検出できる基盤を作る」こと。修正・集約はここでは行わない (Phase 4 で行う)。

1. D-03 対象: 現在の `bloch_display_slot` を含む 4 つのスロット解決の**現挙動**を固定する単体テストを追加する (Local モード / GPU モード × 外部スロット有無。Bloch だけ挙動が違う現状をそのままテストに写す)。これは Q-1 の回答前でも安全に書ける — 現挙動をそのまま固定するだけだから。Q-1 の回答が「バグ」だった場合は、Phase 4 で修正と同時にこのテストの期待値を更新する
2. D-06 対象: `crates/external-gpu-model` に契約値 (Shots::MAX など) の固定テストを追加する

### Phase 2 — 明らかに安全な整理

1. D-09: `#[allow(dead_code)]` の精査と整理 (仕様ページ確認の手順に従う)
2. D-02 補足: `app.rs:33` の `#[allow(unused_imports)]` re-export 精査
3. D-13: progress.md への断り書き追加
4. D-07: Python contract.py の検証ヘルパ抽出
5. D-10: 確定シグナルがある箇所のみ waitForTimeout 置き換え

### Phase 3 — 小さな責務分離 (純粋な code motion)

1. D-02: QniApp フィールドのグループ化 (FpsHud → ExternalGpuState → PickerState 統合 → HoverState の順、各 1 コミット)
2. D-04: gpu/resources 表示系 3 ファイルのサブモジュール分割
3. D-05: circuit_gates.rs の分割

### Phase 4 — 境界とインターフェースの明確化

1. D-01 (A 範囲): `PlacedGate.pos` への書き込み経路の集約と不変条件の文書化
2. D-03: ブロッホのスロット解決を他 3 種と揃える修正 + 4 つの共通ヘルパ集約 (Q-1 でバグ確定済み。Phase 1 のテストが回帰を防ぐ。揃えた後にブロッホのテスト期待値を更新する)
3. D-06 (A 範囲): 外部 GPU 契約の正本表を docs に整備 (Q-3 で「ドキュメント + 固定テストで守る」と確定。スキーマ一本化は Phase 6 の提案のみに留める)

### Phase 5 — 文書の整合

1. D-12: TUI の境界の明文化 (Q-2 で「凍結・文書化のみ」と確定済み。コードは触らない)
2. D-13: `docs/refactor-candidates.html` の進捗注記を、今回完了した項目に合わせて更新
3. `./scripts/lint-docs.sh` 通過を確認

### Phase 6 — 提案のみ (承認なしに実装しない)

以下は設計案・影響範囲・移行手順をまとめて提示するだけに留める:

- D-01 (C 範囲): `PlacedGate.pos` フィールドの完全除去
- D-06 (C 範囲): 契約スキーマの自動生成 / 一本化
- D-11: readback ハンドルの統一
- D-12: TUI のモジュール分割 (今回は凍結。将来必要になったら別タスクとして計画を提示)
- D-08: `test` / `test:pw-legacy` の一本化 (外部自動化の参照を運用側で更新できる目処が立ってから)
- D-14: Python 例外体系とログの刷新

## Verification Requirements (検証の要求)

- 各フェーズ完了時に、そのフェーズで触れた領域の検証コマンドを**新規に** (キャッシュされた過去の成功を流用せず) 実行する
- Rust (apps/web) に触れたら最低限: `cargo clippy --target wasm32-unknown-unknown --tests -- -D warnings` + ネイティブ `cargo test --lib` + `pnpm run test:preflight`。フェーズ完了時は `apps/web/scripts/check-rust.sh`
- 描画・UI に触れたら: `test:bdd` + 関連 Playwright spec + **Playwright での実描画スクリーンショット目視** (`http://127.0.0.1:4174/`、確認後のスクリーンショットは `gio trash` で削除)
- Python に触れたら: unittest 全件。配備関連に触れたら ルート `test-node/` も
- 文書に触れたら: `./scripts/lint-docs.sh`
- 全フェーズ完了時: `./scripts/check-all.sh` をフルで 1 回通し、開発サーバを起動した状態で終える
- 検証は常に「自分の変更を含む最新の状態」で行い、古い実行結果を成果として報告しない

## Reporting Format (報告形式)

作業終了時 (または質問でブロックした時) に、以下を日本語で報告する:

1. **完了した項目**: Debt Map の ID と、対応するコミットハッシュの一覧
2. **ベースラインとの比較**: Phase 0 で記録した検証結果と、最終検証結果の対比
3. **最後に実行したコマンドと結果**: コマンドそのものと、成功 / 失敗・件数を貼る
4. **見送った / 止めた項目**: 理由つき (質問待ち、リスク超過、時間切れ など)
5. **未回答の質問**: Stop And Ask に該当した事項
6. **提案 (Phase 6)**: 提案書の所在 (ファイルパスまたは報告本文)

途中でテストが失敗した場合は、失敗出力をそのまま含めて報告する。成功と言い切れない状態を「たぶん大丈夫」と報告しない。

## Out-of-scope Items (今回やらないこと)

- 機能追加、見た目の変更、パフォーマンス最適化 (リファクタリングの副産物としても狙わない)
- `apps/tui/vendor/ratatui-testlib` の更新・改変 (意図的な vendor。理由は upstream に無い PTY/Sixel テスト機能のため)
- Python 依存バージョン (qiskit / qiskit-aer / numpy) の更新
- `deploy/` 配下と `Dockerfile` の構成変更 (D-14 を実装する場合の最小限の追従を除く)
- CI ワークフローの再設計 (無料枠の問題はコードでは解決しない)
- `AGENTS.md` 冒頭の `../agent-kit/AGENTS.MD` 参照の修正 — このワークスペース (Orca worktree) では壊れているが、正本チェックアウト (`~/Work/qni-webgpu`) では解決する可能性がある。リポジトリ外の資産に関わるため人間の判断に委ねる
- `.mcp.json` の絶対パス (`/home/yasuhito/Work/qni-webgpu/...`) の修正 — 同上
- `cucumber.ts` / `playwright.config.ts` の構成変更 (worker 数、タイムアウト)
- 量子計算のアルゴリズム・シェーダの数値的な変更

## 実装前に確認すべき質問 (全件回答済み)

> Q-1〜Q-4 はすべて回答済みで、本書の各項目に反映済み。実装担当モデルは追加の質問なしに着手してよい (Stop And Ask の一般条件には従う)。記録のため回答内容を残す。

- **Q-1 (D-03)** — 回答: **バグ**。GPU 実行モード中、`bloch_display_slot` だけが他の 3 表示種と違いローカル `gpu_plan` スロットへフォールバックする (`render/circuit_gates.rs:76`) のはバグ。他 3 種と同じく「GPU モードでは外部スロットのみ」に揃える。D-03 を優先度 A のバグ修正として扱う。
- **Q-2 (D-12)** — 回答: **凍結 (文書化のみ)**。apps/tui は独立 CPU PoC として凍結し、境界を docs に明文化するだけ。ファイル分割・重複解消は今回やらない (将来別タスク)。
- **Q-3 (D-06)** — 回答: **ドキュメント + 固定テストで守る**。正本表ドキュメントと両側の固定テストで「片側変更時に気づける」状態にする。スキーマ一本化 (JSON Schema / コード生成) は今回やらず Phase 6 の提案に留める。
- **Q-4 (D-08)** — 回答: **今回は保留**。`test` と `test:pw-legacy` の一本化は、リポジトリ外の自動化 (Symphony / Linear 連携) が `test:pw-legacy` を参照しているため今回は触らない。運用側で外部参照を更新できる目処が立った時点で `test` への一本化に格上げする。
