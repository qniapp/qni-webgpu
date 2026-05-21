# web Cucumber `.feature.md` 導入 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apps/web` に `.feature.md` + `@cucumber/cucumber` + Playwright bridge を導入し、最初の 3 scenario を BDD 化しつつ legacy Playwright suite を並走維持する。

**Architecture:** 初回導入は hybrid 構成に限定する。`apps/web/test-support/browser-launch.cjs` と `apps/web/test-support/web-server.cjs` を shared source of truth として追加し、legacy Playwright と新規 Cucumber 経路の両方が同じ browser launch / server lifecycle を使う。BDD 側は `features/*.feature.md` + `features/support/*.cjs` + `features/step_definitions/*.steps.cjs` で構成し、既存 `web.spec.js` の helper は runner 非依存 helper として段階的に抽出する。

**Tech Stack:** Node.js, `@cucumber/cucumber`, Playwright, `@playwright/test` (legacy runner), trunk, WebGPU, pnpm, GitHub Actions

---

## この pass のファイル構成

- Create: `apps/web/test-support/browser-launch.cjs`
  - executable path 解決
  - flagged WebGPU browser 用 launch policy
  - plain chromium error-path 用 launch policy
- Create: `apps/web/test-support/web-server.cjs`
  - `trunk serve --address 127.0.0.1 --port 4174 --no-autoreload` の shared config
  - URL / timeout / reuse policy
- Create: `apps/web/cucumber.cjs`
  - `.feature.md` 実行設定
- Create: `apps/web/features/startup-success.feature.md`
  - `web canvas renders content` の 1 scenario のみ
- Create: `apps/web/features/plain-chromium-error.feature.md`
  - `default chromium shows a visible WebGPU error instead of a blank page` の 1 scenario のみ
- Create: `apps/web/features/drag-preview-z-order.feature.md`
  - `dragged palette gate stays above the state panel overlay` の 1 scenario のみ
- Create: `apps/web/features/support/world.cjs`
  - Cucumber World state
- Create: `apps/web/features/support/hooks.cjs`
  - browser/context/page lifecycle
  - screenshot / console / pageerror evidence
- Create: `apps/web/features/support/browser.cjs`
  - shared browser-launch policy adapter
- Create: `apps/web/features/support/server.cjs`
  - shared web-server adapter
- Create: `apps/web/features/support/egui-helpers.cjs`
  - Playwright helper 抽出先
- Create (optional only if needed by implementation): `apps/web/features/support/assertions.cjs`
  - custom assertion helper
- Create: `apps/web/test-node/browser-launch.test.cjs`
  - shared browser launch policy test
- Create: `apps/web/test-node/web-server.test.cjs`
  - shared server policy test
- Create: `apps/web/test-node/cucumber-config.test.cjs`
  - cucumber config / scenario-scope guard test
- Modify: `apps/web/package.json`
- Modify: `apps/web/pnpm-lock.yaml`
  - `@cucumber/cucumber` 追加
  - `test:bdd`, `test:pw-legacy` script 追加
  - `test` は初回導入では切り替えない
- Modify: `apps/web/playwright-browser.cjs`
  - shared launcher へ責務移譲、または shared module に委譲する thin wrapper 化
- Modify: `apps/web/playwright.config.cjs`
  - shared `browser-launch.cjs` / `web-server.cjs` を利用
- Modify: `apps/web/test-node/playwright-browser.test.cjs`
  - shared policy 前提に更新
- Modify: `apps/web/test-node/playwright-config.test.cjs`
  - shared webServer policy を検証する形に更新
- Modify: `apps/web/tests/web.spec.js`
  - helper を最小限共通化する場合のみ
  - 初回導入で scenario 自体の削除はしない
- Modify: `docs/web.md`
  - `.feature.md` / `test:bdd` / `test:pw-legacy` / staged rollout を追記
- Modify: `scripts/check-all.sh`
  - staged rollout に沿って `test:bdd` を追加し legacy と並走
- Verify only: `.github/workflows/ci.yml`
  - `./scripts/check-all.sh` 呼び出しは維持。追加変更は原則不要
- Reference spec: `docs/superpowers/specs/2026-04-20-web-cucumber-feature-md-design.md`

## ガードレール

- 実装は **isolated worktree** で行う。
- 開発は **TDD** で進める。新しい共通 policy / Cucumber config / BDD path には先に failing node test または focused scenario を追加する。
- 初回導入で追加してよい `.feature.md` は **3 file / 3 scenario total** のみ。
- 各 `.feature.md` は **1 scenario のみ**とし、`Background` / `Scenario Outline` / 追加 scenario は入れない。
- custom Markdown parser / 変換 pipeline は作らない。`.feature.md` を Cucumber/gherkin の標準サポートで扱う。
- `apps/web` 以外へ Cucumber を広げない。
- browser launch args / executable resolution / server command / URL / timeout / reuse policy は shared module に集約し、legacy Playwright と Cucumber で複製しない。
- 初回導入では legacy Playwright suite (`apps/web/tests/web.spec.js`) を削除しない。
- `package.json` の `test` は初回導入では切り替えない。`test:bdd` と `test:pw-legacy` を併設する。
- `scripts/check-all.sh` に BDD を配線する場合は **BDD + legacy の二段ゲート**とする。
- root workflow の大規模改修は行わない。原則 `scripts/check-all.sh` 経由の変更に留める。
- 実装中は `apps/web` の既存描画・WebGPU 動作を壊さないこと。確認には flagged Chrome 正本運用を維持する。

### Task 0: isolated worktree を準備する

**Files:**

- Modify: none
- Reference: `docs/superpowers/specs/2026-04-20-web-cucumber-feature-md-design.md`
- Reference: `docs/superpowers/plans/2026-04-20-web-cucumber-feature-md.md`

- [ ] **Step 1: 親 working tree が clean であることを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && git status --short
```

Expected:

- no output

- [ ] **Step 2: 既存 worktree / branch が衝突しないことを確認する**

Run:

```bash
cd /home/yasuhito/Work/qni-webgpu && git worktree list
cd /home/yasuhito/Work/qni-webgpu && git branch --list feat/web-cucumber-feature-md
```

Expected:

- `git worktree list` に `web-cucumber-feature-md` がない
- `git branch --list ...` が no output

- [ ] **Step 3: base branch と worktree root を portable に解決する**

Run:

```bash
REPO=/home/yasuhito/Work/qni-webgpu
WORKTREE_ROOT="${QNI_WORKTREE_ROOT:-$HOME/.config/superpowers/worktrees/qni-webgpu}"
BASE_BRANCH="$(git -C "$REPO" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')"
if [ -z "$BASE_BRANCH" ]; then
  BASE_BRANCH="$(git -C "$REPO" rev-parse --abbrev-ref HEAD)"
fi
mkdir -p "$WORKTREE_ROOT"
git -C "$REPO" rev-parse --verify "$BASE_BRANCH"
printf 'BASE_BRANCH=%s\nWORKTREE_ROOT=%s\n' "$BASE_BRANCH" "$WORKTREE_ROOT"
```

Expected:

- `BASE_BRANCH` が空でない
- `WORKTREE_ROOT` が作成される
- `git rev-parse --verify "$BASE_BRANCH"` が success

- [ ] **Step 4: worktree と branch を作成する**

Run:

```bash
REPO=/home/yasuhito/Work/qni-webgpu
WORKTREE_ROOT="${QNI_WORKTREE_ROOT:-$HOME/.config/superpowers/worktrees/qni-webgpu}"
BASE_BRANCH="$(git -C "$REPO" symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null | sed 's#^origin/##')"
if [ -z "$BASE_BRANCH" ]; then
  BASE_BRANCH="$(git -C "$REPO" rev-parse --abbrev-ref HEAD)"
fi
git -C "$REPO" worktree add "$WORKTREE_ROOT/web-cucumber-feature-md" -b feat/web-cucumber-feature-md "$BASE_BRANCH"
```

Expected:

- worktree directory が作成される
- branch `feat/web-cucumber-feature-md` が作成される

- [ ] **Step 5: worktree 側で clean 状態を確認する**

Run:

```bash
WORKTREE_ROOT="${QNI_WORKTREE_ROOT:-$HOME/.config/superpowers/worktrees/qni-webgpu}"
cd "$WORKTREE_ROOT/web-cucumber-feature-md" && git status --short
```

Expected:

- no output

### Task 1: shared browser/server policy を先に切り出し、legacy Playwright を壊さずに共有化する

**Files:**

- Create: `apps/web/test-support/browser-launch.cjs`
- Create: `apps/web/test-support/web-server.cjs`
- Create: `apps/web/test-node/browser-launch.test.cjs`
- Create: `apps/web/test-node/web-server.test.cjs`
- Modify: `apps/web/playwright-browser.cjs`
- Modify: `apps/web/playwright.config.cjs`
- Modify: `apps/web/test-node/playwright-browser.test.cjs`
- Modify: `apps/web/test-node/playwright-config.test.cjs`
- Reference: `docs/superpowers/specs/2026-04-20-web-cucumber-feature-md-design.md`

- [ ] **Step 1: shared policy 用 node test を先に追加する**

追加する test の意図:

- browser launch policy が `PLAYWRIGHT_CHROMIUM_PATH` override と system Chrome 優先を維持する
- flagged WebGPU browser 用 args が shared source of truth になる
- plain chromium error-path 用 mode が shared source of truth になる
- web server policy が `command` / `url` / `timeout` / `reuseExistingServer` を保持する

例:

```js
const test = require('node:test')
const assert = require('node:assert/strict')
const { getWebServerConfig } = require('../test-support/web-server.cjs')

test('shared web server config preserves trunk serve contract', () => {
  const config = getWebServerConfig()
  assert.match(config.command, /trunk serve/)
  assert.equal(config.url, 'http://127.0.0.1:4174')
  assert.equal(config.timeout, 180_000)
  assert.equal(config.reuseExistingServer, true)
})
```

- [ ] **Step 2: test を実行して RED を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && node --test test-node/browser-launch.test.cjs test-node/web-server.test.cjs
```

Expected:

- module not found または export 不足で fail

- [ ] **Step 3: shared policy module を実装する**

実装方針:

- `test-support/browser-launch.cjs`
  - `resolvePlaywrightBrowserExecutable(...)` を shared source of truth にする
  - `getStandardWebGpuLaunchOptions(...)` を用意する
  - `getPlainChromiumLaunchOptions(...)` を用意する
- `test-support/web-server.cjs`
  - `getWebServerConfig()` を export する

shape 例:

```js
function getWebServerConfig() {
  return {
    command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
    url: 'http://127.0.0.1:4174',
    timeout: 180_000,
    reuseExistingServer: true,
  }
}
```

- [ ] **Step 4: legacy Playwright 側を shared policy 利用へ切り替える**

更新対象:

- `playwright-browser.cjs` は shared launcher の thin wrapper または互換 export にする
- `playwright.config.cjs` は `launchOptions` と `webServer` を shared module から構成する

確認観点:

- 現行 flags を落とさない
- 現行 `4174` / `--no-autoreload` / `180_000` / `reuseExistingServer: true` を維持する

- [ ] **Step 5: node preflight を再実行して GREEN を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:preflight
```

Expected:

- shared policy test を含めて pass

- [ ] **Step 6: legacy Playwright の focused baseline を再確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec playwright test --grep 'web canvas renders content|default chromium shows a visible WebGPU error instead of a blank page|dragged palette gate stays above the state panel overlay'
```

Expected:

- 3 tests すべて pass

- [ ] **Step 7: Task 1 を commit する**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md add \
  apps/web/test-support/browser-launch.cjs \
  apps/web/test-support/web-server.cjs \
  apps/web/test-node/browser-launch.test.cjs \
  apps/web/test-node/web-server.test.cjs \
  apps/web/playwright-browser.cjs \
  apps/web/playwright.config.cjs \
  apps/web/test-node/playwright-browser.test.cjs \
  apps/web/test-node/playwright-config.test.cjs

git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md commit -m "test: share web browser and server policy"
```

Expected:

- shared policy commit が作成される

### Task 2: Cucumber runner と support 基盤を追加する

**Files:**

- Create: `apps/web/cucumber.cjs`
- Create: `apps/web/features/support/world.cjs`
- Create: `apps/web/features/support/hooks.cjs`
- Create: `apps/web/features/support/browser.cjs`
- Create: `apps/web/features/support/server.cjs`
- Create: `apps/web/features/support/egui-helpers.cjs`
- Create (if needed): `apps/web/features/support/assertions.cjs`
- Create: `apps/web/test-node/cucumber-config.test.cjs`
- Modify: `apps/web/package.json`
- Modify: `apps/web/pnpm-lock.yaml`
- Reference: `apps/web/tests/web.spec.js`
- Reference: `docs/superpowers/specs/2026-04-20-web-cucumber-feature-md-design.md`

- [ ] **Step 1: config / script contract の failing test を先に追加する**

`test-node/cucumber-config.test.cjs` は **必須** とし、少なくとも以下を固定する。

- `package.json` に `test:bdd` と `test:pw-legacy` がある
- `test` は初回導入では Playwright のまま維持される
- `cucumber.cjs` が `.feature.md` glob と support/step definitions を読む

例:

```js
test('package scripts add bdd and keep legacy primary test', async () => {
  const path = require('node:path')
  const pkg = JSON.parse(await fs.readFile(path.join(__dirname, '..', 'package.json'), 'utf8'))
  assert.equal(pkg.scripts['test'], 'playwright test')
  assert.equal(pkg.scripts['test:pw-legacy'], 'playwright test')
  assert.match(pkg.scripts['test:bdd'], /cucumber-js/)
})
```

- [ ] **Step 2: test を実行して RED を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && node --test test-node/cucumber-config.test.cjs
```

Expected:

- script / config 未実装で fail

- [ ] **Step 3: `package.json` と `cucumber.cjs` を追加する**

実装方針:

- `@cucumber/cucumber` を devDependencies に追加
- script を追加:

```json
{
  "scripts": {
    "test": "playwright test",
    "test:pw-legacy": "playwright test",
    "test:bdd": "cucumber-js --config cucumber.cjs",
    "test:preflight": "node --test test-node/*.test.cjs"
  }
}
```

- `cucumber.cjs` は `.feature.md` のみを拾う

shape 例:

```js
module.exports = {
  paths: ['features/**/*.feature.md'],
  require: ['features/step_definitions/**/*.cjs', 'features/support/**/*.cjs'],
  publishQuiet: true,
  failFast: true,
}
```

- [ ] **Step 4: dependency を install して lockfile を更新する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm install
```

Expected:

- `@cucumber/cucumber` を含む install が成功する
- `pnpm-lock.yaml` が更新される

- [ ] **Step 5: support skeleton を追加する**

役割:

- `world.cjs`: browser/context/page/error buffer
- `browser.cjs`: shared browser-launch adapter
- `server.cjs`: shared web-server adapter
- `hooks.cjs`: Before/After setup/teardown + screenshot evidence
- `egui-helpers.cjs`: 既存 helper の抽出先

この段階では full helper 実装でなくてよい。まず runner 起動に必要な骨格だけを作る。

- [ ] **Step 6: node config test を再実行して GREEN にする**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:preflight
```

Expected:

- Cucumber config を含む node tests が pass

- [ ] **Step 7: temp smoke feature で runner/config/support load を deterministic に確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web
TMPDIR="$(mktemp -d)"
cat > "$TMPDIR/smoke.feature.md" <<'EOF'
Feature: cucumber config smoke
  Scenario: runner loads config and support
    Given a smoke noop step
EOF
cat > "$TMPDIR/smoke.steps.cjs" <<'EOF'
const { Given } = require('@cucumber/cucumber')
Given('a smoke noop step', function () {})
EOF
pnpm exec cucumber-js --config cucumber.cjs --dry-run --require "$TMPDIR/smoke.steps.cjs" "$TMPDIR/smoke.feature.md"
rm -rf "$TMPDIR"
```

Expected:

- exit 0
- 1 scenario が検出される
- config parse error / support load error が出ない

- [ ] **Step 8: Task 2 を commit する**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md add \
  apps/web/package.json \
  apps/web/pnpm-lock.yaml \
  apps/web/cucumber.cjs \
  apps/web/features/support/world.cjs \
  apps/web/features/support/hooks.cjs \
  apps/web/features/support/browser.cjs \
  apps/web/features/support/server.cjs \
  apps/web/features/support/egui-helpers.cjs \
  apps/web/test-node/cucumber-config.test.cjs

git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md commit -m "test: add web cucumber runner scaffolding"
```

Expected:

- Cucumber scaffolding commit が作成される

### Task 3: startup success scenario を `.feature.md` へ移植する

**Files:**

- Create: `apps/web/features/startup-success.feature.md`
- Create: `apps/web/features/step_definitions/startup-success.steps.cjs`
- Modify: `apps/web/features/support/egui-helpers.cjs`
- Reference: `apps/web/tests/web.spec.js`

- [ ] **Step 1: `.feature.md` と step file を先に追加する**

内容:

- 1 scenario のみ
- flagged WebGPU browser で open → initialize → visible canvas → error absent → initial state vector 確認

例:

```md
Feature: web startup success

  Scenario: WebGPU canvas renders content with the standard browser
    Given the web app is open in the standard WebGPU browser
    When the app finishes initializing
    Then the WebGPU error is absent
    And the canvas is visible
    And the initial state vector is:
      | 1 |
      | 0 |
      | 0 |
      | 0 |
```

- [ ] **Step 2: focused BDD を実行して RED を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec cucumber-js --config cucumber.cjs features/startup-success.feature.md
```

Expected:

- undefined step または helper 未実装で fail

- [ ] **Step 3: 必要な helper / step を最小実装する**

抽出候補:

- `readStateVector(...)`
- app ready wait
- `#egui-canvas` 可視確認
- `window.__eguiError` 取得

方針:

- 既存 `web.spec.js` の処理を runner 非依存 helper として移す
- `@playwright/test` の `expect.poll` に依存しない retry helper を使う

- [ ] **Step 4: focused BDD を再実行して GREEN を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec cucumber-js --config cucumber.cjs features/startup-success.feature.md
```

Expected:

- 1 scenario pass

- [ ] **Step 5: legacy Playwright の同等 scenario が壊れていないことを確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec playwright test --grep 'web canvas renders content'
```

Expected:

- pass

- [ ] **Step 6: Task 3 を commit する**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md add \
  apps/web/features/startup-success.feature.md \
  apps/web/features/step_definitions/startup-success.steps.cjs \
  apps/web/features/support/egui-helpers.cjs

git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md commit -m "test: add web startup bdd scenario"
```

Expected:

- startup success BDD commit が作成される

### Task 4: plain chromium visible error scenario を移植する

**Files:**

- Create: `apps/web/features/plain-chromium-error.feature.md`
- Create: `apps/web/features/step_definitions/plain-chromium-error.steps.cjs`
- Modify: `apps/web/features/support/browser.cjs`
- Modify: `apps/web/features/support/egui-helpers.cjs`
- Reference: `apps/web/tests/web.spec.js`

- [ ] **Step 1: 1 scenario だけの `.feature.md` を追加する**

例:

```md
Feature: web plain chromium error

  Scenario: Plain chromium shows a visible error instead of a blank page
    Given the web app is open in plain chromium
    When the app finishes initializing
    Then a visible WebGPU error message is shown
```

- [ ] **Step 2: focused BDD を実行して RED を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec cucumber-js --config cucumber.cjs features/plain-chromium-error.feature.md
```

Expected:

- plain chromium mode の step / helper 不足で fail

- [ ] **Step 3: plain chromium launch path と visible error assertion を最小実装する**

確認観点:

- shared launcher policy の plain chromium mode を使う
- `window.__eguiError` または visible error DOM を使って白画面回避仕様を検証する
- launch args を step 側に複製しない

- [ ] **Step 4: focused BDD を再実行して GREEN を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec cucumber-js --config cucumber.cjs features/plain-chromium-error.feature.md
```

Expected:

- 1 scenario pass

- [ ] **Step 5: legacy Playwright の同等 scenario が壊れていないことを確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec playwright test --grep 'default chromium shows a visible WebGPU error instead of a blank page'
```

Expected:

- pass

- [ ] **Step 6: Task 4 を commit する**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md add \
  apps/web/features/plain-chromium-error.feature.md \
  apps/web/features/step_definitions/plain-chromium-error.steps.cjs \
  apps/web/features/support/browser.cjs \
  apps/web/features/support/egui-helpers.cjs

git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md commit -m "test: add web plain chromium error bdd scenario"
```

Expected:

- plain chromium BDD commit が作成される

### Task 5: drag preview z-order scenario を移植する

**Files:**

- Create: `apps/web/features/drag-preview-z-order.feature.md`
- Create: `apps/web/features/step_definitions/drag-preview-z-order.steps.cjs`
- Modify: `apps/web/features/support/egui-helpers.cjs`
- Modify: `apps/web/features/support/assertions.cjs` (if needed)
- Reference: `apps/web/tests/web.spec.js`

- [ ] **Step 1: 1 scenario だけの `.feature.md` を追加する**

例:

```md
Feature: web drag preview z-order

  Scenario: Dragged palette gate stays above the state panel overlay
    Given the web app is open in the standard WebGPU browser
    And the app finishes initializing
    When I drag the X gate from the palette over the state panel
    Then the dragged gate stays above the state panel overlay
```

- [ ] **Step 2: focused BDD を実行して RED を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec cucumber-js --config cucumber.cjs features/drag-preview-z-order.feature.md
```

Expected:

- drag / pixel sampling / assertion 未実装で fail

- [ ] **Step 3: drag helper と pixel assertion を最小実装する**

抽出候補:

- `dragPointer(...)`
- screenshot retry helper
- `sampleCanvasPixels(...)`
- state panel overlay との前後比較 helper

方針:

- 既存 Playwright test の座標/比較ロジックを `egui-helpers.cjs` に寄せる
- 既存仕様を変えない
- BDD scenario のために drag semantics を外へ露出しすぎない

- [ ] **Step 4: focused BDD を再実行して GREEN を確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec cucumber-js --config cucumber.cjs features/drag-preview-z-order.feature.md
```

Expected:

- 1 scenario pass

- [ ] **Step 5: legacy Playwright の同等 scenario が壊れていないことを確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm exec playwright test --grep 'dragged palette gate stays above the state panel overlay'
```

Expected:

- pass

- [ ] **Step 6: Task 5 を commit する**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md add \
  apps/web/features/drag-preview-z-order.feature.md \
  apps/web/features/step_definitions/drag-preview-z-order.steps.cjs \
  apps/web/features/support/egui-helpers.cjs \
  apps/web/features/support/assertions.cjs

git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md commit -m "test: add web drag preview bdd scenario"
```

Expected:

- drag preview BDD commit が作成される

### Task 6: staged rollout を package/docs/check-all に配線する

**Files:**

- Modify: `apps/web/package.json`
- Modify: `docs/web.md`
- Modify: `scripts/check-all.sh`
- Verify only: `.github/workflows/ci.yml`

- [ ] **Step 1: rollout contract を先に書く**

追記内容:

- `test:bdd` の目的
- `test:pw-legacy` を残す理由
- 初回導入では `test` を切り替えないこと
- `scripts/check-all.sh` では BDD + legacy を並走させること

- [ ] **Step 2: `check-all.sh` を staged rollout に合わせて更新する**

形の例:

```bash
echo "==> Web: Playwright preflight (browser resolution)"
pnpm -C "$ROOT_DIR/apps/web" run test:preflight

echo "==> Web: Cucumber BDD"
pnpm -C "$ROOT_DIR/apps/web" run test:bdd

echo "==> Web: Playwright legacy"
pnpm -C "$ROOT_DIR/apps/web" run test:pw-legacy
```

- [ ] **Step 3: docs を更新する**

`docs/web.md` に追記する内容:

- `.feature.md` が初回導入されたこと
- BDD path と legacy path の使い分け
- 初回導入では 3 scenario のみが BDD 化されていること
- browser/server shared policy を通じて flagged Chrome 正本運用を維持していること

- [ ] **Step 4: shell / node / focused BDD + legacy をまとめて確認する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md && bash -n scripts/check-all.sh
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:preflight
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:bdd
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:pw-legacy
```

Expected:

- 4 コマンドすべて success

- [ ] **Step 5: Task 6 を commit する**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md add \
  apps/web/package.json \
  docs/web.md \
  scripts/check-all.sh

git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md commit -m "test: wire web bdd rollout"
```

Expected:

- rollout wiring commit が作成される

### Task 7: full verification / mechanical checks / review-ready 状態にする

**Files:**

- Modify: none unless fixes are required
- Reference: `apps/web/**`
- Reference: `scripts/check-all.sh`
- Reference: `docs/web.md`

- [ ] **Step 1: full verification を実行する**

Run:

```bash
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:preflight
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:bdd
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && pnpm run test:pw-legacy
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web && env PATH="$HOME/.cargo/bin:$PATH" trunk build
cd ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md && git diff --check
```

Expected:

- 5 コマンドすべて success

- [ ] **Step 2: scenario scope / anti-drift / unchanged-surface を機械確認する**

Run:

```bash
python - <<'PY'
from pathlib import Path
root = Path('/home/yasuhito/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md/apps/web')
feature_root = root / 'features'
features = sorted(p.relative_to(feature_root).as_posix() for p in feature_root.rglob('*.feature.md'))
assert features == [
    'drag-preview-z-order.feature.md',
    'plain-chromium-error.feature.md',
    'startup-success.feature.md',
], features
texts = [(feature_root / rel).read_text() for rel in features]
assert sum(t.count('Scenario:') for t in texts) == 3
assert all('Scenario Outline:' not in t for t in texts)
assert all('Background:' not in t for t in texts)
config = (root / 'playwright.config.cjs').read_text()
assert 'test-support/browser-launch.cjs' in config
assert 'test-support/web-server.cjs' in config
hooks = (root / 'features/support/browser.cjs').read_text() + (root / 'features/support/server.cjs').read_text()
assert 'test-support/browser-launch.cjs' in hooks
assert 'test-support/web-server.cjs' in hooks
print('BDD_PLAN_MECHANICAL_OK')
PY
```

Expected:

- `BDD_PLAN_MECHANICAL_OK`

- [ ] **Step 3: review 用 summary を整える**

まとめるべき内容:

- shared policy module 追加
- 3 `.feature.md` / 3 scenario 導入
- legacy Playwright retention
- `check-all.sh` staged rollout
- docs update
- verification evidence

- [ ] **Step 4: Task 7 を commit する（修正がある場合のみ）**

Run:

```bash
git -C ~/.config/superpowers/worktrees/qni-webgpu/web-cucumber-feature-md status --short
```

Expected:

- clean、または review fix がある場合のみ追加 commit を作る

## 実行後レビュー観点

reviewer には少なくとも以下を確認してもらう。

- 初回導入が 3 scenario total に厳密に収まっているか
- legacy Playwright と Cucumber が shared browser/server policy を実際に共有しているか
- browser flag drift / server lifecycle drift がないか
- `test` を prematurely 切り替えていないか
- `scripts/check-all.sh` が BDD + legacy staged rollout になっているか
- `.feature.md` の実装に custom parser や cross-app scope creep がないか

## 完了条件

- `apps/web` に `.feature.md` + `@cucumber/cucumber` の実行入口が追加されている
- `apps/web/pnpm-lock.yaml` が依存追加を反映している
- shared source of truth として
  - `apps/web/test-support/browser-launch.cjs`
  - `apps/web/test-support/web-server.cjs`
  が存在する
- legacy Playwright と Cucumber support code の両方が shared module を参照している
- 初回導入の `.feature.md` は exactly 3 file / 3 scenario total である
- `Background` / `Scenario Outline` / 追加 scenario がない
- `test:bdd` と `test:pw-legacy` が存在する
- `test` は初回導入では `playwright test` のまま維持される
- `scripts/check-all.sh` が BDD + legacy の staged rollout を実行する
- `docs/web.md` が新しい test path を説明している
- `pnpm run test:preflight` が pass
- `pnpm run test:bdd` が pass
- `pnpm run test:pw-legacy` が pass
- `env PATH="$HOME/.cargo/bin:$PATH" trunk build` が pass
- `git diff --check` が pass
