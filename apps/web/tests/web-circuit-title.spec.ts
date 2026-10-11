import { expect, test } from '@playwright/test'
import { waitForStartupReady } from './support/web-spec-helpers'

// Issue #59: qni circuit JSON may carry a root `title`, which qni shows as
// the page title and keeps in the circuit URL.
const titledHash = (json: string): string => `/#${encodeURIComponent(json)}`

test('circuit title from the URL names the page', async ({ page }) => {
  await page.goto(titledHash('{"cols":[["H"]],"title":"Superdense Coding"}'))
  await waitForStartupReady(page)

  await expect(page).toHaveTitle('Superdense Coding')
})

test('circuit without a title keeps the app name as the page title', async ({ page }) => {
  await page.goto(titledHash('{"cols":[["H"]]}'))
  await waitForStartupReady(page)

  await expect(page).toHaveTitle('Qni')
})

test('circuit URL keeps the trimmed title after the cols', async ({ page }) => {
  await page.goto(titledHash('{"title":"  Superdense Coding ","cols":[["H"]]}'))
  await waitForStartupReady(page)

  await expect.poll(() => decodeURIComponent(new URL(page.url()).hash))
    .toBe('#{"cols":[["H"]],"title":"Superdense Coding"}')
})
