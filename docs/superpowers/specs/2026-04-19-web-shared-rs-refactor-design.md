# web `shared.rs` 抽出設計（thin-entry 次パス）

## 背景

- `apps/web/src/lib.rs` は thin-entry pass の完了により、`gates.rs` / `colors.rs` 抽出後に 140 LOC まで縮小した。
- 現在の `lib.rs` に残る主責務は次の 3 系統である。
  - 共有定数
  - shared helper（`now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba`）
  - wasm export（`start`, `read_state_vector`）と module wiring
- 依存の実態としては、これら helper はすでに root 実装詳細というより、`app.rs` / `render.rs` / `colors.rs` から参照される内部共有 API になっている。
- ユーザーは次の thin-entry 候補として、**A: helper だけを pure extraction する**方針を選択した。`color_rgba` を `colors.rs` に寄せる境界整理は今回は行わない。

## 目的

- `lib.rs` から shared helper を `shared.rs` へ安全に分離し、crate root をさらに thin-entry 化する。
- helper 利用側を `crate::shared::...` の直接参照に揃え、root alias 依存を取り除く。
- 挙動変更なしの code motion に徹し、既存 UI / WebGPU / drag 挙動を一切変えない。

## 非目的

- 共有定数の再配置。
- `PALETTE_GATES` の移動。
- `start` / `read_state_vector` の移動や公開面変更。
- `gates.rs` / `colors.rs` / `app.rs` / `render.rs` の責務再編。
- `color_rgba` を `colors.rs` に寄せる追加整理。
- helper の命名変更、アルゴリズム変更、最適化。
- UI / Playwright / WebGPU 挙動の変更。
- repo-wide CI 相当 aggregate check の導入。

## 採用方針

この pass では **`shared.rs` を新設し、helper だけを pure extraction する**。

移動対象:

- `now_seconds`
- `display_index_to_state_index`
- `amplitude_qubits`
- `color_rgba`

この pass では次の「ついで変更」は行わない。

- `color_rgba` を `colors.rs` の private helper にする
- 定数を `shared.rs` / `colors.rs` / `render.rs` へ再配置する
- wasm export を別 module に移す
- `shared.rs` を public API や root re-export の受け皿にする

## 比較した案

### 案A: `shared.rs` に helper だけを pure extraction（採用）

- `apps/web/src/shared.rs` に helper を集約する。
- 利点:
  - 現在の残存責務に対して最小リスクで効く。
  - `lib.rs` が constants + exports + module wiring へさらに近づく。
  - 利用側の依存先が明確になる。
- 欠点:
  - internal module が 1 つ増える。
  - helper 群の概念的まとまりは弱めで、将来さらに再整理する余地が残る。

### 案B: `color_rgba` も含めて色境界を整理する

- `color_rgba` を `colors.rs` に寄せ、時間・state helper だけ `shared.rs` に置く。
- 利点:
  - `colors.rs` がより self-contained になる。
- 欠点:
  - 今回のユーザー選択 A（pure extraction only）から外れる。
  - 変更量が増え、単純な code motion ではなくなる。

### 案C: helper と定数をまとめて再編する

- `shared.rs` / `layout.rs` / `render.rs` などへ定数も関連配置する。
- 利点:
  - `lib.rs` をさらに薄くできる。
- 欠点:
  - スコープが膨らみ、今回の low-risk pass に向かない。
  - constants ownership まで再設計が必要になる。

## この pass のモジュール境界

### 新規追加: `apps/web/src/shared.rs`

ここには、crate 内で横断利用される shared helper を移す。

対象:

- `now_seconds`
- `display_index_to_state_index`
- `amplitude_qubits`
- `color_rgba`

意図:

- `lib.rs` から内部実装 helper を取り除く。
- `app.rs` / `render.rs` / `colors.rs` の参照先を明示化する。

### `apps/web/src/lib.rs` に残すもの

対象:

- 共有定数
- `PALETTE_GATES`
- wasm export（`start`, `read_state_vector`）
- module wiring（`mod app; mod colors; mod gates; mod gpu; mod icons; mod layout; mod render; mod shared;`）

意図:

- `lib.rs` を crate root / thin-entry として保つ。
- 今回は constants と wasm export を root に残し、中間段階として許容する。

## 依存関係の方針

この pass の主目的は、**shared helper の実装位置を root から internal module へ移し、利用側を direct module path に揃えること**である。

想定する依存:

- `app.rs` → `crate::shared::now_seconds`
- `render.rs` → `crate::shared::{amplitude_qubits, display_index_to_state_index}`
- `colors.rs` → `crate::shared::color_rgba`
- `lib.rs` → `shared.rs`（module declaration のみ）

方針:

- `crate::now_seconds` / `crate::amplitude_qubits` / `crate::display_index_to_state_index` / `crate::color_rgba` の root path は残さない。
- root re-export（`pub use shared::...`）は追加しない。
- helper 利用側は `crate::shared::...` を直接参照する。
- `gates.rs` / `gpu.rs` / `icons.rs` など、今回 helper を使っていない module は無変更とする。

## 可視性の方針

- 外部公開（`pub`）は追加しない。
- `shared.rs` に移す helper は crate 内利用に必要な最小限として **`pub(crate)`** とする。
- `shared.rs` 内に新規 helper を足さない。
- root re-export（`pub use`, `pub(crate) use`, `pub(super) use`）は追加しない。

## `start` / `read_state_vector` の扱い

wasm export の公開面は動かさない。

採用方針:

- `#[wasm_bindgen] pub async fn start(...)` は `lib.rs` に残す。
- `#[wasm_bindgen] pub async fn read_state_vector(...)` は `lib.rs` に残す。
- 関数名・シグネチャ・公開位置は維持する。

## 実装ガードレール

- 移動対象 helper 本体は、module path と visibility 調整を除き原則そのまま移す。
- 挙動変更につながる refactor はしない。
- `shared.rs` は internal module に留め、public API としない。
- 定数は `lib.rs` に残す。
- `PALETTE_GATES` は `lib.rs` に残す。
- `start` / `read_state_vector` は `lib.rs` に残す。
- tests は変更しない。
- root re-export は追加しない。
- 旧 root path 参照は残さない。

## 実装手順

1. `apps/web/src/shared.rs` を追加し、`lib.rs` に `mod shared;` を宣言する。
2. `now_seconds` / `display_index_to_state_index` / `amplitude_qubits` / `color_rgba` を `shared.rs` に移す。
3. `app.rs` / `render.rs` / `colors.rs` の import / 参照を `crate::shared::...` 前提に更新する。
4. 早い段階で `cargo check --target wasm32-unknown-unknown` を実行して import / visibility 崩れを切り分ける。
5. Playwright / trunk build / diff check を再実行し、挙動差分がないことを確認する。
6. 最後に symbol move / remains / no-root-reexport / no-old-root-path / tests unchanged / LOC を確認する。

## 受け入れ条件

- `apps/web/src/shared.rs` が追加されている。
- `lib.rs` から以下の helper が移っている。
  - `now_seconds`
  - `display_index_to_state_index`
  - `amplitude_qubits`
  - `color_rgba`
- `lib.rs` には上記 helper の定義が残っていない。
- `lib.rs` には共有定数、`PALETTE_GATES`、wasm export、module wiring が残っている。
- `app.rs` / `render.rs` / `colors.rs` は `crate::shared::...` を直接参照している。
- root re-export は追加されていない。
- `cargo check --target wasm32-unknown-unknown` が通る。
- `pnpm exec playwright test` が通る。
- `trunk build` が通る。
- `git diff --check` が通る。
- `apps/web/tests` と `apps/web/test-node` は未変更である。
- `lib.rs` の LOC が current head（thin-entry pass 完了後）よりさらに減っている。

## 検証

実装後は少なくとも以下を再実行する。

```bash
cd /home/yasuhito/Work/qni-webgpu/apps/web && cargo check --target wasm32-unknown-unknown
cd /home/yasuhito/Work/qni-webgpu/apps/web && pnpm exec playwright test
cd /home/yasuhito/Work/qni-webgpu/apps/web && trunk build
cd /home/yasuhito/Work/qni-webgpu && git diff --check
```

この pass の web 検証系には `cargo insta` ベースの snapshot suite は含まれていないため、snapshot 確認は追加しない。UI 回帰は既存 Playwright を正本とする。
また、この pass の spec では CI 相当の repo-wide aggregate check（例: `./scripts/check-all.sh`）は **スコープ外** とする。理由は、今回の変更対象が `apps/web` の局所的な code motion であり、これまでの pass と同様に wasm cargo check + Playwright + trunk build + diff check を正本の受け入れ条件とするためである。

追加で、shared helper move を機械的に確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/web/src/lib.rs').read_text()
shared = Path('apps/web/src/shared.rs').read_text()
for pattern in [
    r'fn now_seconds',
    r'fn display_index_to_state_index',
    r'fn amplitude_qubits',
    r'fn color_rgba',
]:
    if re.search(pattern, lib):
        raise SystemExit(f'helper still present in lib.rs: {pattern}')
    if not re.search(pattern, shared):
        raise SystemExit(f'helper missing from shared.rs: {pattern}')
print('ok')
PY
```

期待値:

- `ok`

追加で、利用側が agreed mapping どおり direct module path を使っていることを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
checks = {
    'apps/web/src/app.rs': [
        'use crate::shared::now_seconds;',
    ],
    'apps/web/src/render.rs': [
        'use crate::shared::{amplitude_qubits, display_index_to_state_index};',
    ],
    'apps/web/src/colors.rs': [
        'crate::shared::color_rgba(',
    ],
}
for path, needles in checks.items():
    text = Path(path).read_text()
    for needle in needles:
        if needle not in text:
            raise SystemExit(f'missing expected shared mapping in {path}: {needle}')
print('ok')
PY
```

期待値:

- `ok`

追加で、root re-export がないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
for path in Path('apps/web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'^(pub\s+use|pub\(crate\)\s+use|pub\(super\)\s+use)\s+(shared::|self::shared::|crate::shared::|shared::\{|self::\{[^}]*\bshared::|crate::\{[^}]*\bshared::)', text, re.M | re.S):
        raise SystemExit(f'forbidden root re-export in {path}')
print('ok')
PY
```

期待値:

- `ok`

追加で、旧 root path が残っていないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
for path in Path('apps/web/src').glob('*.rs'):
    text = path.read_text()
    if re.search(r'crate::(now_seconds|display_index_to_state_index|amplitude_qubits|color_rgba)\b', text):
        raise SystemExit(f'forbidden old root path in {path}')
print('ok')
PY
```

期待値:

- `ok`

追加で、`lib.rs` に残すべきシンボルが残っており、定数が `shared.rs` に漏れていないことを確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && python - <<'PY'
from pathlib import Path
import re
lib = Path('apps/web/src/lib.rs').read_text()
shared = Path('apps/web/src/shared.rs').read_text()
lib_patterns = [
    r'const REM\b',
    r'const STATE_CIRCLE_SIZE\b',
    r'const STATE_CIRCLE_GAP\b',
    r'const STATE_CIRCLE_BOTTOM_MARGIN\b',
    r'const STATE_CIRCLE_STROKE\b',
    r'const MIN_QUBITS\b',
    r'const MAX_QUBITS\b',
    r'const MAX_STATE_COUNT\b',
    r'const LINE_Y\b',
    r'const LINE_GAP\b',
    r'const CIRCUIT_PADDING\b',
    r'const QUBIT_LABEL_WIDTH\b',
    r'const QUBIT_LABEL_GAP\b',
    r'const LINE_LEFT_OFFSET\b',
    r'const LINE_RIGHT_OFFSET\b',
    r'const GATE_SIZE\b',
    r'const SLOT_SPACING\b',
    r'const SNAP_DISTANCE\b',
    r'const DRAG_REPAINT_BASE_SECS\b',
    r'const DRAG_REPAINT_MIN_SECS\b',
    r'const DRAG_REPAINT_MAX_SECS\b',
    r'const DRAG_REPAINT_PUMP_FACTOR\b',
    r'const PALETTE_SIZE\b',
    r'const PALETTE_GAP\b',
    r'const PALETTE_ROW_Y\b',
    r'const PALETTE_GATES\b',
    r'pub async fn start\b',
    r'pub async fn read_state_vector\b',
]
for pattern in lib_patterns:
    if not re.search(pattern, lib):
        raise SystemExit(f'missing expected lib.rs symbol: {pattern}')
    if re.search(pattern, shared):
        raise SystemExit(f'symbol moved into shared.rs unexpectedly: {pattern}')
if re.search(r'^const\s+', shared, re.M):
    raise SystemExit('shared.rs unexpectedly contains const definitions')
print('ok')
PY
```

期待値:

- `ok`

追加で、tests 未変更を確認する。

```bash
cd /home/yasuhito/Work/qni-webgpu && git status --short --untracked-files=all -- apps/web/tests apps/web/test-node
```

期待値:

- 出力なし

## リスクと対策

- import 漏れ / visibility 漏れ
  - 対策: 早い段階で wasm `cargo check` を回し、task を細かく切り分ける。
- old root path の取り残し
  - 対策: `rg` / Python による mechanical check を受け入れ条件に含める。
- `shared.rs` が convenience root alias の温床になる
  - 対策: root re-export 禁止を明示し、機械的に検査する。
- `lib.rs` に残すべき定数や wasm export を誤って動かす
  - 対策: remains check を受け入れ条件に含める。

## 完了時の期待状態

- `lib.rs` は constants + `PALETTE_GATES` + wasm exports + module wiring を中心とした thin-entry に一段近づく。
- internal helper 実装は `shared.rs` にまとまり、利用側は `crate::shared::...` を直接参照する。
- 挙動変更なしのまま、将来の crate root 整理を続けやすい状態になる。
