/**
 * Playwright teardown project — cleans up test data after e2e-ui tests.
 * Runs ONCE after all e2e-ui tests complete.
 */
import { test as teardown } from '@playwright/test';

const API = 'http://localhost:7700';
const H = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'fj_devtestadminkey000000',
  'Content-Type': 'application/json',
};

teardown('cleanup test data', async ({ request }) => {
  // Delete seeded test index
  await request.delete(`${API}/1/indexes/e2e-products`, { headers: H }).catch(() => {});

  // Delete temp indexes that tests may have created
  await request.delete(`${API}/1/indexes/e2e-temp`, { headers: H }).catch(() => {});

  // Clear analytics for test index
  await request.delete(`${API}/2/analytics/clear`, {
    headers: H,
    params: { index: 'e2e-products' },
  }).catch(() => {});
});
