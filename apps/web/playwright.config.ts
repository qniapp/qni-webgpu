import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests',
  use: {
    baseURL: 'http://127.0.0.1:4173',
    viewport: { width: 1000, height: 800 },
    browserName: 'chromium',
    headless: process.env.HEADLESS !== '0',
    launchOptions: {
      args: [
        '--enable-features=WebGPU,WebGPUDeveloperFeatures,WebGPUService,Vulkan',
        '--enable-unsafe-webgpu',
        '--enable-dawn-features=allow_unsafe_apis,enable_immediate_error_handling',
        '--ignore-gpu-blocklist',
        '--disable-gpu-sandbox',
        '--no-sandbox',
        '--use-gl=angle',
        '--use-angle=swiftshader',
        '--use-vulkan=swiftshader',
      ],
    },
  },
  webServer: {
    command: 'pnpm exec vite --host 127.0.0.1 --port 4173 --strictPort',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: true,
  },
})
