import { test as base } from '@playwright/test';

/**
 * Custom test fixture that pre-seeds localStorage with Flapjack auth credentials.
 * Import { test, expect } from this module instead of '@playwright/test' to get
 * an authenticated page automatically.
 */
export const test = base.extend({
  page: async ({ page }, use) => {
    await page.addInitScript(() => {
      localStorage.setItem('flapjack-api-key', 'abcdef0123456789');
      localStorage.setItem('flapjack-app-id', 'flapjack');
      // Seed the Zustand persist store so useAuth().apiKey is populated on hydration
      localStorage.setItem('flapjack-auth', JSON.stringify({
        state: { apiKey: 'abcdef0123456789', appId: 'flapjack' },
        version: 0,
      }));
    });
    await use(page);
  },
});

export { expect } from '@playwright/test';
