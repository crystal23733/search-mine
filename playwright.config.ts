import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: true,
  workers: process.env.CI ? 2 : 4,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  use: { baseURL: 'http://127.0.0.1:5173', trace: 'retain-on-failure' },
  webServer: {
    command: 'pnpm --filter @liar/web dev',
    url: 'http://127.0.0.1:5173',
    reuseExistingServer: !process.env.CI,
  },
  projects: [
    { name: 'chromium', testIgnore: '**/board-metrics.spec.ts', use: { ...devices['Desktop Chrome'] } },
    { name: 'mobile', testIgnore: '**/board-metrics.spec.ts', use: { ...devices['Pixel 7'] } },
    { name: 'measurement', testMatch: '**/board-metrics.spec.ts', dependencies: ['chromium', 'mobile'], use: { ...devices['Desktop Chrome'] } },
  ],
});
