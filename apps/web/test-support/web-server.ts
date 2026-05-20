type WebServerConfig = {
  url: string
  timeout: number
  reuseExistingServer: boolean
} & (
  | {
      command: string
      external?: false
    }
  | {
      external: true
      command?: never
    }
)
type PlaywrightWebServerConfig = Extract<WebServerConfig, { command: string }>

type WebServerRequest = {
  env?: NodeJS.ProcessEnv
}

export const PLAYWRIGHT_EXTERNAL_SERVER_ENV = 'QNI_WEB_EXTERNAL_SERVER'
export const PLAYWRIGHT_BASE_URL_ENV = 'QNI_WEB_BASE_URL'
const DEFAULT_WEB_SERVER_URL = 'http://127.0.0.1:4174'
const DEFAULT_WEB_SERVER_TIMEOUT_MS = 180_000

export const getWebServerConfig = ({ env = process.env }: WebServerRequest = {}): WebServerConfig => {
  if (env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] === '1') {
    return {
      url: env[PLAYWRIGHT_BASE_URL_ENV] || DEFAULT_WEB_SERVER_URL,
      timeout: DEFAULT_WEB_SERVER_TIMEOUT_MS,
      reuseExistingServer: true,
      external: true,
    }
  }

  return {
    command: 'env -u NO_COLOR TRUNK_COLOR=never trunk serve --address 127.0.0.1 --port 4174 --no-autoreload',
    url: DEFAULT_WEB_SERVER_URL,
    timeout: DEFAULT_WEB_SERVER_TIMEOUT_MS,
    reuseExistingServer: true,
  }
}

export const getPlaywrightBaseUrl = ({ env = process.env }: WebServerRequest = {}): string =>
  getWebServerConfig({ env }).url

export const getPlaywrightWebServerConfig = ({
  env = process.env,
}: WebServerRequest = {}): PlaywrightWebServerConfig | undefined =>
  env[PLAYWRIGHT_EXTERNAL_SERVER_ENV] === '1'
    ? undefined
    : getWebServerConfig({ env }) as PlaywrightWebServerConfig
