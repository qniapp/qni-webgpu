# egui-web `lib.rs` thin-entry 化設計（次パス）

## 背景
- `apps/egui-web/src/lib.rs` は pass 1（`layout.rs` / `icons.rs`）、pass 2（`gpu.rs`）、pass 3（`render.rs`）、pass 4（`app.rs`）の抽出後、367 LOC まで縮小した。
- pass 4 完了後の `lib.rs` は、主に次の責務を持っている。
  - 共有定数
  - `now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba`
  - gate/domain helper (`GateKind`, `GateMatrix`, `GateParams`, `gate_matrix`, `gate_params`, `gate_params_controlled`)
  - `Colors`
  - wasm export (`start`, `read_state_vector`)
- これにより、`lib.rs` の残存責務はすでに「crate root wiring + 共有 helper + 残った内部実装」に近づいているが、まだ **gate/domain helper と `Colors` の実装詳細が同居**している。
- ユーザーは次候補について、**A: thin-entry pass** を選択した。さらに詳細として、**A1: helper / `Colors` だけを移し、定数は `lib.rs` に残す**ことを選択した。
- したがって今回は、`lib.rs` を薄い入口ファイルへ近づけるため、gate/domain helper と `Colors` を専用 internal module に安全に分離し、定数・wasm export・共有 helper は `lib.rs` に残す。

## 目的
- `lib.rs` から gate/domain helper と `Colors` を安全に分離し、crate root を thin-entry に近づける。
- 挙動変更なしの code motion に徹し、既存の `app.rs` / `render.rs` / `gpu.rs` / `layout.rs` / `icons.rs` から参照される共有シンボルの位置だけを整理する。
- 将来の `lib.rs` 最終整理、あるいは shared/domain/theme 境界の再検討をしやすくする。

## 非目的
- 共有定数の再配置。
- `now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba` の移動。
- wasm export (`start`, `read_state_vector`) の移動や公開面変更。
- `app.rs` / `render.rs` / `gpu.rs` の責務再編。
- `Colors` の API 変更。
- gate/domain helper の仕様変更や最適化。
- UI / drag / WebGPU 挙動の変更。
- この pass だけで全ファイルを 500 LOC 以下に揃えること。

## 採用方針
この pass では **gate/domain helper と `Colors` だけを internal module へ移す**。ユーザー選択どおり、定数は `lib.rs` に残し、次のような「ついで変更」は行わない。

- 共有定数の theme / layout / gate 別再編
- `start` / `read_state_vector` の移動
- `now_seconds` / `amplitude_qubits` など shared helper の整理
- `Colors` のフィールド名変更や theme API 化
- gate/domain helper の引数整理
- `crate::GateKind` などの root re-export 導入

これにより、今回の pass は「`lib.rs` は crate root wiring・共有定数・共有 helper・wasm export を持ち、gate/domain helper と `Colors` の実装詳細は internal module にある」という責務線を作ることに限定する。

## 比較した案

### 案A: `gates.rs` + `colors.rs` の 2 module 分割（採用）
- `apps/egui-web/src/gates.rs`
  - `GateKind`
  - `GateMatrix`
  - `GateParams`
  - `gate_matrix`
  - `gate_params`
  - `gate_params_controlled`
- `apps/egui-web/src/colors.rs`
  - `Colors`
- 利点:
  - gate/domain helper と色定義の責務が自然に分かれる。
  - `app.rs` / `render.rs` / `gpu.rs` がどの共有概念を使っているかが読みやすい。
  - 将来、theme 整理や domain 整理をするときの受け皿になる。
- 欠点:
  - module 数は 2 つ増える。
  - `lib.rs` 直下の internal module 構成が少し増える。

### 案B: `core.rs` など 1 module にまとめる
- gate/domain helper と `Colors` を 1 ファイルにまとめる。
- 利点:
  - module 数を増やしすぎない。
- 欠点:
  - UI/theme と gate/domain の責務が再び混ざる。
  - 次の整理対象が曖昧なまま残る。

### 案C: helper / `Colors` に加えて定数も再配置する
- 案Aに加えて、gate / layout / state-circle / drag の定数も関連 module へ寄せる。
- 利点:
  - `lib.rs` をもっと薄くできる。
- 欠点:
  - ユーザー選択 A1（定数は残す）と合わない。
  - 変更量とリスクが増える。

## この pass のモジュール境界

### 新規追加: `apps/egui-web/src/gates.rs`
ここには、**gate/domain helper に閉じた責務**を移す。

対象:
- `GateKind`
- `GateMatrix`
- `GateParams`
- `gate_matrix`
- `gate_params`
- `gate_params_controlled`

意図:
- gate/domain に関する shared logic を 1 module に閉じ込める。
- `app.rs` / `gpu.rs` / `render.rs` から参照される domain 概念を明確にする。

### 新規追加: `apps/egui-web/src/colors.rs`
ここには、**色定義に閉じた責務**を移す。

対象:
- `Colors`

意図:
- `Colors` の実装詳細を `lib.rs` から切り離す。
- UI/theme 寄りの shared type として分離し、将来の theme 整理をしやすくする。

### `apps/egui-web/src/lib.rs` に残すもの
対象:
- 共有定数
- `now_seconds`
- `display_index_to_state_index`
- `amplitude_qubits`
- `color_rgba`
- wasm export (`start`, `read_state_vector`)
- `mod app; mod render; mod gpu; mod layout; mod icons; mod gates; mod colors;`

意図:
- `lib.rs` を crate root / thin-entry に近づける。
- 今回は共有 helper と定数を残し、中間段階として許容する。

## 依存関係の方針
この pass の主目的は **`lib.rs` から gate/domain helper と `Colors` を externalize すること**である。純粋抽出を成立させるため、既存 module は root re-export を経由せず、直接 internal module を参照する形へ切り替える。

想定する依存:
- `lib.rs` → `app.rs`
- `lib.rs` → `gates.rs`
- `lib.rs` → `colors.rs`
- `app.rs` → `crate::gates::{...}` / `crate::colors::Colors`
- `render.rs` → `crate::gates::GateKind` / `crate::colors::Colors`
- `gpu.rs` → `crate::gates::GateParams` / `crate::colors::Colors`
- `icons.rs` → `crate::gates::GateKind` / `crate::colors::Colors`

方針:
- 今回は **依存の完全最適化より、安全な抽出を優先** する。
- `crate::GateKind` / `crate::GateParams` / `crate::Colors` のような root 参照は、**直接 `crate::gates::...` / `crate::colors::...` へ切り替える**。
- この意図は `app.rs` / `render.rs` / `gpu.rs` だけでなく、**`icons.rs` を含む moved symbol 利用側すべて**に適用する。
- `pub use gates::...` や `pub use colors::...` のような root re-export は追加しない。
- 既存 module の責務は変えず、型参照先と import だけを最小限更新する。

## 可視性の方針
- `gates.rs` / `colors.rs` に移すシンボルは、crate 内から使える最小限の visibility とする。
- 外部公開 (`pub`) は追加しない。必要なら **`pub(crate)` に統一** する。
- `GateKind` / `GateParams` / `gate_params` / `gate_params_controlled` / `Colors` / `Colors::new` は既存 sibling module から参照されるため、**必要最小限で `pub(crate)` とする**。
- `GateKind::label()` は `icons.rs` から呼ばれるため、**`pub(crate)` とする**。
- `Colors` の field は `render.rs` / `gpu.rs` / `icons.rs` から直接読まれるため、cross-module access に必要な field だけ **`pub(crate)` とする**。
- `GateMatrix` と `gate_matrix` は `gates.rs` 内で閉じられるなら private のままにする。
- root re-export (`pub use`, `pub(crate) use`, `pub(super) use`) は追加しない。

## `start` / `read_state_vector` の扱い
wasm export の公開面は動かさない。

採用方針:
- `#[wasm_bindgen] pub async fn start(...)` は `lib.rs` に残す。
- `#[wasm_bindgen] pub async fn read_state_vector(...)` は `lib.rs` に残す。
- 関数名・シグネチャ・公開位置は維持する。

## 実装ガードレール
この pass は挙動変更なしの抽出に限定し、以下を原則とする。

- 移動対象の enum / struct / fn / impl 本体は、import 解決と最小限の visibility 調整を除いて原則そのまま移す。
- 可視性変更は **必要最小限** に限定し、外部公開 (`pub`) は追加しない。
- 定数はこの pass では `lib.rs` に残す。
- `now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba` は `lib.rs` に残す。
- `app.rs` / `render.rs` / `gpu.rs` の責務は変更しない。
- `start` / `read_state_vector` の wasm export は変更しない。
- root re-export は追加しない。
- 命名変更、最適化、コメント整理などの「ついで変更」はしない。

## 実装手順
1. `apps/egui-web/src/gates.rs` と `apps/egui-web/src/colors.rs` を追加し、`lib.rs` に `mod gates; mod colors;` を宣言する。
2. `GateKind`, `GateMatrix`, `GateParams`, `gate_matrix`, `gate_params`, `gate_params_controlled` を `gates.rs` に移す。
3. `Colors` を `colors.rs` に移す。
4. `app.rs` / `render.rs` / `gpu.rs` / `icons.rs` / `lib.rs` の import を `crate::gates::...` / `crate::colors::...` 前提に最小限更新する。
5. `cargo check --target wasm32-unknown-unknown` を早い段階で実行して import / visibility 崩れを切り分ける。
6. 既存 Playwright / trunk build / diff check を再実行し、挙動差分がないことを確認する。
7. 最後に symbol move / remains check / no-root-reexport check / tests unchanged / LOC を確認する。

## 受け入れ条件
- `apps/egui-web/src/gates.rs` と `apps/egui-web/src/colors.rs` が追加されている。
- `lib.rs` から以下の定義が移っている。
  - `GateKind`
  - `GateMatrix`
  - `GateParams`
  - `gate_matrix`
  - `gate_params`
  - `gate_params_controlled`
  - `Colors`
- `lib.rs` には上記の定義が残っていない。
- 共有定数・共有 helper・wasm export は `lib.rs` に残っている。
- `app.rs` / `render.rs` / `gpu.rs` / `icons.rs` は `crate::gates::...` / `crate::colors::...` を直接参照している。
- root re-export は追加されていない。
- `cargo check --target wasm32-unknown-unknown` が通る。
- `pnpm exec playwright test` が通る。
- `trunk build` が通る。
- `git diff --check` が通る。
- `apps/egui-web/tests` と `apps/egui-web/test-node` は未変更である。
- `lib.rs` の LOC が pass 4 完了時より減っている。

## 検証
実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/egui-web && trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

この pass の egui-web 検証系には `cargo insta` ベースの snapshot suite は含まれていないため、snapshot 確認は追加しない。UI 回帰は既存 Playwright を正本とする。
また、この pass の spec では CI 相当の repo-wide aggregate check（例: `./scripts/check-all.sh`）は **スコープ外** とする。理由は、今回の変更対象が `apps/egui-web` の局所的な code motion であり、これまでの pass と同様に wasm cargo check + Playwright + trunk build + diff check を正本の受け入れ条件とするためである。

追加で、gate/theme symbol move を機械的に確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'enum GateKind|struct GateMatrix|struct GateParams|fn gate_matrix|fn gate_params\(|fn gate_params_controlled|struct Colors' apps/egui-web/src/lib.rs apps/egui-web/src/gates.rs apps/egui-web/src/colors.rs
```

期待値:
- gate/domain helper は `gates.rs` 側にある
- `Colors` は `colors.rs` 側にある
- `lib.rs` にはそれらの定義が残っていない

追加で、既存 module が root ではなく直接 internal module を参照していることを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && rg -n 'crate::gates::|crate::colors::|use crate::gates::|use crate::colors::' apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/gpu.rs apps/egui-web/src/icons.rs apps/egui-web/src/lib.rs
```

期待値:
- `app.rs` / `render.rs` / `gpu.rs` / `icons.rs` / `lib.rs` は `gates.rs` / `colors.rs` を直接参照している
- `crate::GateKind` / `crate::GateParams` / `crate::Colors` のような root 経由参照は残っていない

追加で、root re-export がないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
files = [
    'apps/egui-web/src/lib.rs',
    'apps/egui-web/src/app.rs',
    'apps/egui-web/src/render.rs',
    'apps/egui-web/src/gpu.rs',
    'apps/egui-web/src/icons.rs',
    'apps/egui-web/src/colors.rs',
    'apps/egui-web/src/gates.rs',
]
for path in files:
    text = Path(path).read_text()
    if re.search(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+(gates::|colors::|self::gates::|self::colors::|crate::gates::|crate::colors::|gates::\{|colors::\{|self::\{[^}]*\b(gates::|colors::)|crate::\{[^}]*\b(gates::|colors::))', text, re.M | re.S):
        raise SystemExit(f'forbidden root re-export in {path}')
print('ok')
PY
```

期待値:
- `ok`

追加で、forbidden root path が残っていないことも確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
for path in Path('apps/egui-web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'use\s+crate::\{[^}]*\b(GateKind|GateParams|Colors)\b[^}]*\}', text, re.S):
        raise SystemExit(f'forbidden root grouped import in {path}')
    if re.search(r'crate::(GateKind|GateParams|Colors)\b', text):
        raise SystemExit(f'forbidden root path reference in {path}')
print('ok')
PY
```

期待値:
- grouped import を含め、forbidden root alias / root path reference は残っていない
- moved symbol 利用側は root alias ではなく `crate::gates::...` / `crate::colors::...` を直接参照している

また、テストファイルが変更されていないことも確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && git diff --name-only -- apps/egui-web/tests apps/egui-web/test-node
```

期待値:
- no output

LOC 確認として以下も実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu && wc -l apps/egui-web/src/lib.rs apps/egui-web/src/gates.rs apps/egui-web/src/colors.rs apps/egui-web/src/app.rs apps/egui-web/src/render.rs apps/egui-web/src/gpu.rs apps/egui-web/src/layout.rs apps/egui-web/src/icons.rs
```

## 次パスの判断基準
この thin-entry pass の完了後、`lib.rs` には主に次の責務が残る想定である。

- crate root wiring
- 共有定数
- `now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba`
- wasm export

したがって、次候補は次のルールで決める。

- 共有 helper がまだ大きいなら → shared utility module への抽出
- crate root が十分薄くなったなら → 分割シリーズ完了

現時点の見立てでは、この pass 完了後の `lib.rs` はかなり thin-entry に近づき、分割シリーズの締めどころを判断しやすくなる。