# web Cucumber `.feature.md` 導入設計

## 背景

- `apps/web` の Web テストは現在 `@playwright/test` による **Playwright 直書き** で運用している。
- 実行入口は次の 3 箇所に揃っている。
  - `apps/web/package.json` の `"test": "playwright test"`
  - `scripts/check-all.sh` の `pnpm -C "$ROOT_DIR/apps/web" exec playwright test`
  - `.github/workflows/ci.yml` の `./scripts/check-all.sh`
- 既存の Playwright suite には、単純な DOM 確認だけでなく、以下のような **描画/ピクセル比較系 helper** が多数含まれる。
  - `waitForStateVectorReady(...)`
  - `readStateVector(...)`
  - `sampleCanvasPixels(...)`
  - `dragPointer(...)`
  - screenshot retry / canvas decode / pixel diff 系 helper
- ユーザーは、Web テストを **Cucumber の Markdown 方式**で扱えるかを確認し、`.feature.md` の公式サポート有無を根拠付きで再確認するよう求めた。
- 調査の結果、以下を確認した。
  - `cucumber/gherkin` の `MARKDOWN_WITH_GHERKIN.md` にて、**Markdown with Gherkin (MDG) は Gherkin parser にサポートされる**と明記されている。
  - 同文書にて、**MDG file は `.feature.md` 拡張子を使う必要がある**と明記されている。
  - `cucumber-js` では PR #1645 と changelog/release にて、**Markdown support（experimental support for Markdown）** が追加されたことが確認できる。
- したがって、この repo でも **`.feature.md` を正本とする Cucumber 導入は技術的に可能**である。

## 目的

- `apps/web` の Web テストに **`.feature.md` + `@cucumber/cucumber`** を導入し、ユーザー視点のシナリオ記述を Markdown with Gherkin へ移す。
- ただし、既存のブラウザ起動条件・WebGPU フラグ・描画検証 helper は捨てず、**Playwright をブラウザ実行基盤として再利用**する。
- 最初の導入は小さく始め、**代表 3 シナリオを段階移行**できる設計境界を定める。
- 既存の CI / ローカル検証フローに対して、変更点を最小限に留める。

## 非目的

- 既存 `apps/web/tests/web.spec.js` を一度に全面削除すること。
- `@playwright/test` 依存 helper を一気に全面書き換えること。
- ブラウザ選択ロジックや WebGPU フラグ方針を変更すること。
- `apps/web` 以外のテスト体系（`apps/tui` など）へ Cucumber を広げること。
- `.feature.md` とは別の独自 Markdown DSL を設計すること。
- CI を大規模に再設計すること。

## 採用方針

この導入は **hybrid 構成**で進める。

- 仕様記述: `features/**/*.feature.md`
- 実行 runner: `@cucumber/cucumber`
- ブラウザ制御/画面検証: Playwright ライブラリ API
- 既存ブラウザ解決ロジック: `apps/web/playwright-browser.cjs` を再利用
- 既存 Playwright 直書き suite: 導入初期は **legacy 経路として残す**

要するに、
**「テストを Cucumber に置き換える」のではなく、「仕様層を Cucumber/MDG に持ち上げ、ブラウザ検証層は Playwright を維持する」**
方針を採る。

## 比較した案

### 案A: Playwright 直書きのまま整理を続ける

- 利点:
  - 最小コスト。
  - 既存 helper / retry / screenshot 資産をそのまま使える。
- 欠点:
  - ユーザーが求める `.feature.md` 方式にならない。
  - ユーザー視点シナリオの可読性向上という目的を満たさない。

### 案B: `.feature.md` + `@cucumber/cucumber` + Playwright bridge（採用）

- 利点:
  - `.feature.md` を正式に採用できる。
  - 既存ブラウザ起動条件と描画検証 helper を活かせる。
  - 段階移行がしやすく、CI 変更を最小化できる。
- 欠点:
  - `@playwright/test` runner 依存 helper を、Cucumber から呼べる形へ少しずつ整理する必要がある。
  - runner が二重化する移行期間が発生する。

### 案C: Cucumber-first に全面置換する

- 利点:
  - テスト入口が完全に統一される。
- 欠点:
  - 現在の helper 群を広範囲に書き換える必要があり、初回導入コストが高い。
  - 既存の 13 本の Playwright suite を一度に移すのはリスクが高い。

## モジュール/ファイル境界

### 追加するもの

想定する追加ファイル群:

- `apps/web/cucumber.cjs`
- `apps/web/features/startup-success.feature.md`
- `apps/web/features/plain-chromium-error.feature.md`
- `apps/web/features/drag-preview-z-order.feature.md`
- `apps/web/features/step_definitions/*.steps.cjs`
- `apps/web/features/support/world.cjs`
- `apps/web/features/support/hooks.cjs`
- `apps/web/features/support/browser.cjs`
- `apps/web/features/support/server.cjs`
- `apps/web/features/support/egui-helpers.cjs`
- `apps/web/test-support/browser-launch.cjs`
- `apps/web/test-support/web-server.cjs`
- 必要に応じて `features/support/assertions.cjs`

初期導入で追加してよい feature file は、**上記 3 scenario に直接対応するものだけ**とする。`state vector` や他の drag visual 群を別 file へ先回りで分解するのは、この pass のスコープ外とする。

### 維持・再利用するもの

- `apps/web/playwright-browser.cjs`
  - 既存の executable path 解決ロジックは shared launcher から再利用する
- `apps/web/playwright.config.cjs`
  - legacy Playwright 経路をしばらく維持する場合は残す
  - ただし browser args / channel policy / executable resolution / webServer policy の source of truth は shared module 側へ寄せる
- `apps/web/test-node/playwright-browser.test.cjs`
- `apps/web/test-node/playwright-config.test.cjs`
  - preflight / config 検証は当面そのまま維持

### 既存 Playwright suite の扱い

- 初期導入では `apps/web/tests/web.spec.js` をすぐ削除しない。
- まずは Cucumber 側に **代表 3 シナリオだけ移植**し、その後に全面移行判断を行う。
- helper の共通化に合わせて、将来的に `web.spec.js` からロジックを段階的に切り出す。

## 実行モデル

### Cucumber runner

`@cucumber/cucumber` を使って `features/**/*.feature.md` を実行する。

`cucumber.cjs` では少なくとも以下を定義する想定:

- feature glob: `features/**/*.feature.md`
- require:
  - `features/step_definitions/**/*.cjs`
  - `features/support/**/*.cjs`
- fail-fast や publish 無効化など、CI 向けの安定設定

### Server orchestration

新しい `test:bdd` 経路でも、現在の Playwright と同じ server lifecycle を明示的に共有する。

shared source of truth:

- `apps/web/test-support/web-server.cjs`

ここには少なくとも以下を置く。

- server command: `env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload`
- server URL: `http://127.0.0.1:4174`
- startup timeout: 現行 `180_000` を維持
- reuse policy: 既存 server が居れば再利用できる設計にする

方針:

- `pnpm -C apps/web run test:bdd` は ad hoc に server を起動しない。
- Cucumber 側は `features/support/server.cjs` から shared `web-server.cjs` を使って起動/待機/tear down を行う。
- `playwright.config.cjs` の `webServer` も同じ shared config を参照し、`4174` / `--no-autoreload` / timeout の drift を防ぐ。

### World

`features/support/world.cjs` では、各 scenario ごとに次を保持する。

- browser
- context
- page
- base URL
- console error / page error の収集状態
- reusable helper API

### Hooks

`features/support/hooks.cjs` では、少なくとも以下を行う。

- `Before`: browser/context/page 初期化
- `After`: page/context/browser cleanup
- failure 時 screenshot 保存
- failure 時 console/pageerror の要約付与

### Browser launcher

`features/support/browser.cjs` では、以下を担う。

- shared launcher policy から browser launch option を受け取る
- フラグ付き Google Chrome 正本運用の踏襲
- 標準 WebGPU browser と plain chromium の 2 モード起動切替

shared source of truth:

- `apps/web/test-support/browser-launch.cjs`

ここには少なくとも以下を集約する。

- `playwright-browser.cjs` を通した executable path 解決
- `PLAYWRIGHT_CHROMIUM_PATH` override の扱い
- 標準 WebGPU browser 用 launch args
- plain chromium error path 用 launch args / mode

方針:

- `features/support/browser.cjs` は launch arg を独自に複製しない。
- `playwright.config.cjs` も同じ shared launcher policy を参照し、legacy Playwright と Cucumber 間で WebGPU flag drift を起こさない。
- 既存 `test-node` preflight は executable lookup だけで終わらせず、shared launch policy の主要値を検証対象に含められる設計にする。

## シナリオ移行の初期スコープ

初回導入では、次の 3 本を対象にする。

1. `web canvas renders content`
2. `default chromium shows a visible WebGPU error instead of a blank page`
3. `dragged palette gate stays above the state panel overlay`

ガードレール:

- 初回導入で追加してよい `Scenario` block は **合計 3 本のみ**とする。
- 各 `.feature.md` は **1 scenario のみ**を持つ。
- `Background` / `Scenario Outline` / 追加 scenario は初回導入では入れない。

理由:

- 成功系を 1 本含む
- plain browser failure 系を 1 本含む
- drag / overlay / pixel comparison 系を 1 本含む

これにより、導入初期で以下がすべて検証できる。

- browser 起動ポリシー
- app 初期化待ち
- state vector readback
- screenshot/pixel sampling
- drag helper
- error-visible 化の仕様

## Step 定義の方針

step は内部実装語ではなく、**ユーザー/仕様視点の語彙**で書く。

例:

- `Given the web app is open in the standard WebGPU browser`
- `Given the web app is open in plain chromium`
- `When the app finishes initializing`
- `When I drag the X gate from the palette into the circuit`
- `Then the WebGPU error is absent`
- `Then a visible WebGPU error message is shown`
- `Then the dragged gate stays above the state panel overlay`

方針:

- step file に低レベル座標計算や screenshot decode を直接書き込まない。
- pixel comparison / retry / drag 操作は support helper に押し込む。
- step は「何を確かめるか」に寄せ、「どうやるか」は helper に閉じ込める。

## 既存 helper の整理方針

現在の `web.spec.js` には、Cucumber にそのまま持ち込みにくい `@playwright/test` runner 寄り helper がある。
そのため、導入時は以下の 3 分類で整理する。

### そのまま pure helper 化できるもの

- `dragPointer(...)`
- `releasePointer(...)`
- `readStateVector(...)`
- `sampleCanvasPixels(...)`
- screenshot retry helper の内部ロジック

### 自前 retry helper に寄せるもの

- `expect.poll(...)` 依存の待機系 helper
- `waitForStateVectorReady(...)`
- `waitForStateVectorApprox(...)`
- `waitForCanvasBlue(...)`

### assertion 層を薄く包み直すもの

- `expect(...)` に強く依存する比較を、Node `assert` または小さな custom assertion へ寄せる

設計原則:

- helper は runner 非依存に寄せる
- assertion は step か support assertion helper へ閉じ込める
- scenario の意味論は `.feature.md` に寄せる

## package.json / script 方針

導入初期は **並走期間あり** とする。

想定 script:

- `test:bdd`: `cucumber-js`
- `test:pw-legacy`: `playwright test`
- `test:preflight`: 現状維持

初期段階では、`test` を即座に `test:bdd` に切り替える必要はない。
安全策として、rollout は次の順で固定する。

1. `test:bdd` を追加する。
2. 初回 CI / `check-all.sh` 配線を入れる場合は、`test:bdd` を **legacy `test:pw-legacy` と並走**させる。
3. 代表 3 scenario が安定し、後続 pass で追加承認が取れた時点でのみ、`test` や `check-all.sh` の primary entrypoint 切替を検討する。

## CI / ローカル検証方針

CI の骨格は大きく変えない。

維持するもの:

- `pnpm install`
- Playwright browser install
- `test:preflight`
- flagged Chrome を前提にした browser resolution 方針

変更候補:

- `scripts/check-all.sh` の Web 実行部を、段階的に
  - まず `pnpm run test:bdd` を追加
  - 初期導入中は `pnpm run test:pw-legacy` と並走
  - 後続 pass でのみ primary 切替を検討

導入初期の推奨:

- まずは `test:bdd` を追加し、legacy を残す
- CI を触る場合も当面は **BDD + legacy の二段ゲート**にする
- green を確認したうえで、別 pass として Web test の正本入口切替可否を判断する

## エラーハンドリング / デバッグ方針

Cucumber 導入で失ってはいけないのは、現在の WebGPU / canvas failure の調査容易性である。

そのため、失敗時には少なくとも次を残す。

- screenshot
- console errors
- page errors
- `window.__eguiError` の内容
- plain chromium / flagged browser のどちらで走ったか

方針:

- runner を変えても、失敗証拠量は減らさない。
- 特に pixel comparison 系 scenario では、差分が追えない失敗形式にしない。

## 実装ガードレール

- `.feature.md` を使うが、独自 Markdown 変換パイプラインは作らない。
- browser 解決ロジックは shared launcher policy を通じて `playwright-browser.cjs` を再利用し、重複実装しない。
- WebGPU launch args と `plain chromium` モードは shared launcher policy に集約し、legacy Playwright と Cucumber で複製しない。
- server command / URL / timeout / reuse policy は shared web-server module に集約し、legacy Playwright と Cucumber で複製しない。
- 既存の Playwright helper を一度に全面移植しない。
- 初回導入は代表 3 シナリオに限定する。
- 初回導入では Scenario は合計 3 本のみとし、各 `.feature.md` は 1 scenario のみ、`Background` / `Scenario Outline` / 追加 scenario を入れない。
- `apps/web/tests/web.spec.js` は初回導入では削除しない。
- CI の入口を即日全面切り替えしない。
- 既存の `test-node` preflight は保持する。
- plain chromium failure scenario と flagged Chrome success scenario の両方を維持する。
- 変更は `apps/web` と必要最小限の root script / CI 配線に留める。

## 受け入れ条件

- `apps/web` に `@cucumber/cucumber` ベースの実行入口が追加されている。
- `features/**/*.feature.md` で MDG シナリオを実行できる。
- `playwright-browser.cjs` を Cucumber 側でも再利用している。
- 代表 3 シナリオが `.feature.md` + step definitions へ移されている。
- 代表 3 シナリオ実行時に、既存の browser flag 方針が維持されている。
- 失敗時 screenshot / console / page error の証拠が残る。
- legacy Playwright 経路が、初回導入時点では必要に応じて温存されている。
- `scripts/check-all.sh` / CI の変更がある場合でも、Web test 実行経路が壊れていない。
- `playwright.config.cjs` が `apps/web/test-support/browser-launch.cjs` と `apps/web/test-support/web-server.cjs` を参照している。
- Cucumber 側 support code も同じ shared module を参照している。
- shared module を使うことで browser args / executable resolution / `webServer.command` / `url` / `timeout` / `reuseExistingServer` の source of truth が 1 箇所になっている。

## 検証

導入時には少なくとも以下を確認する。

### ローカル

- `pnpm -C apps/web run test:preflight`
- `pnpm -C apps/web run test:bdd`
- 必要なら `pnpm -C apps/web run test:pw-legacy`

### CI 相当

- `.github/workflows/ci.yml` と `scripts/check-all.sh` の Web test 入口整合
- Playwright browser install 後に Cucumber 経路が実行できること

### anti-drift / shared policy

- node-level test で `apps/web/test-support/browser-launch.cjs` の主要 launch policy を検証すること
- node-level test で `apps/web/test-support/web-server.cjs` の `command` / `url` / `timeout` / `reuseExistingServer` を検証すること
- legacy Playwright と Cucumber support code の両方が shared module を import していることを確認すること

## レビュー観点

reviewer には少なくとも以下を確認してもらう。

- `.feature.md` 採用の根拠と `cucumber-js` の境界が spec 上で明確か
- Playwright browser policy を壊さない構成になっているか
- helper 移行の単位が大きすぎないか
- 初回導入スコープが 3 シナリオに制限されているか
- legacy suite を即削除しないガードレールが十分か
- CI 変更が過剰になっていないか

## 参考根拠

- `cucumber/gherkin` `MARKDOWN_WITH_GHERKIN.md`
  - MDG は Gherkin parser でサポートされる
  - MDG file は `.feature.md` 拡張子を使う
- `cucumber/cucumber-js` PR #1645
  - Markdown support の導入履歴
- `cucumber/cucumber-js` CHANGELOG / v7.3.0 release
  - Experimental support for Markdown
