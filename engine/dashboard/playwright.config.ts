import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration for Flapjack dashboard.
 *
 * Three test categories:
 * - e2e-ui: Real browser + real server, simulated-human interaction (no mocks)
 * - e2e-api: API-level tests against real server (no browser rendering)
 * - seed/cleanup: Setup/teardown projects for e2e-ui data seeding
 *
 * @see https://playwright.dev/docs/test-configuration
 */
export default defineConfig({
  testDir: './tests',
  globalSetup: './tests/global-setup.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : 3,
  reporter: 'html',

  use: {
    baseURL: 'http://localhost:5177',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    // --- Setup: seed test data into real backend ---
    {
      name: 'seed',
      testDir: './tests/e2e-ui',
      testMatch: 'seed.setup.ts',
      teardown: 'cleanup',
    },
    // --- Teardown: delete test data ---
    {
      name: 'cleanup',
      testDir: './tests/e2e-ui',
      testMatch: 'cleanup.setup.ts',
    },
    // --- E2E-UI: real browser + real server, no mocks ---
    {
      name: 'e2e-ui',
      testDir: './tests/e2e-ui',
      testIgnore: ['*.setup.ts'],
      dependencies: ['seed'],
      use: { ...devices['Desktop Chrome'] },
    },
    // --- E2E-API: API-level tests against real server (no browser rendering) ---
    // For real-browser simulated-human tests, see the e2e-ui project above.
    {
      name: 'e2e-api',
      testDir: './tests/e2e-api',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5177',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
