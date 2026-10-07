import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: true,
  workers: process.env.CI ? 2 : 4,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  use: { baseURL: "http://127.0.0.1:5173", trace: "retain-on-failure" },
  webServer: [
    {
      command: "pnpm --filter @liar/web dev",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: !process.env.CI,
    },
    {
      command:
        "pnpm --filter @liar/web exec vite preview --host 127.0.0.1 --port 4173 --strictPort",
      url: "http://127.0.0.1:4173",
      reuseExistingServer: !process.env.CI,
    },
  ],
  projects: [
    {
      name: "chromium",
      testIgnore: ["**/board-metrics.spec.ts", "**/offline.spec.ts"],
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "mobile",
      testIgnore: ["**/board-metrics.spec.ts", "**/offline.spec.ts"],
      use: { ...devices["Pixel 7"] },
    },
    {
      name: "offline-chromium",
      testMatch: "**/offline.spec.ts",
      use: { ...devices["Desktop Chrome"], baseURL: "http://127.0.0.1:4173" },
    },
    {
      name: "offline-mobile",
      testMatch: "**/offline.spec.ts",
      use: { ...devices["Pixel 7"], baseURL: "http://127.0.0.1:4173" },
    },
    {
      name: "measurement",
      testMatch: "**/board-metrics.spec.ts",
      dependencies: [
        "chromium",
        "mobile",
        "offline-chromium",
        "offline-mobile",
      ],
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
