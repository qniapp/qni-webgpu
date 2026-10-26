import type { Page } from '@playwright/test'

/** Chromium interventions can appear only in CDP Log, not page console events. */
export async function collectBrowserErrors(page: Page) {
  const messages: unknown[] = []
  page.on('console', message => {
    if (message.type() === 'error') messages.push({ channel: 'console', text: message.text() })
  })
  page.on('pageerror', error => messages.push({ channel: 'pageerror', text: error.message }))
  const cdp = await page.context().newCDPSession(page)
  cdp.on('Log.entryAdded', ({ entry }) => {
    if (entry.level === 'error' || entry.source === 'intervention') messages.push({ channel: 'Log', ...entry })
  })
  await cdp.send('Log.enable')
  return { messages, cdp }
}
