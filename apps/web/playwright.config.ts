import { defineConfig } from '@playwright/test'
import { chromium } from 'playwright'
import { getStandardWebGpuLaunchOptions } from './test-support/browser-launch'
import {
  getPlaywrightBaseUrl,
  getPlaywrightWebServerConfig,
} from './test-support/web-server'

const webServer = getPlaywrightWebServerConfig({ env: process.env })
const standardBrowser = getStandardWebGpuLaunchOptions({
  env: process.env,
  defaultPath: chromium.executablePath(),
})
// Chrome を並列に多数起動すると、WebGPU のデバイス取得が応答しないまま止まる
// ページが混ざる (実測: 6 並列で 1〜2 回の実行に 1 件、3 並列では 4 回の実行で 0 件)。
// 起動が固まった場合は `bootstrap.ts` の監視が明示的なエラーへ切り替えるが、
// 発生自体を減らすため並列数を抑える。
const workers = 3

// 上記の起動固まりは Chrome / Dawn 側の待ちで、こちらから解消できない。
// 単一ページの Cucumber でも発生するため並列数だけでは消えない。
// 1 回だけ再試行し、2 回続けて落ちるものは本当の退行として扱う
// (再試行で通ったものは flaky として報告される)。
const retries = 1

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  workers,
  retries,
  use: {
    baseURL: getPlaywrightBaseUrl({ env: process.env }),
    viewport: { width: 1000, height: 800 },
    browserName: 'chromium',
    headless: standardBrowser.headless,
    launchOptions: {
      executablePath: standardBrowser.executablePath,
      args: standardBrowser.args,
    },
  },
  webServer,
})
