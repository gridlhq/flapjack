/**
 * Playwright setup project — seeds test data into the real Flapjack backend.
 * Runs ONCE before any e2e-ui tests.
 *
 * Requires: Flapjack server running on port 7700
 */
import { test as setup, expect } from '@playwright/test';
import { PRODUCTS, SYNONYMS, RULES, SETTINGS } from '../fixtures/test-data';

const API = 'http://localhost:7700';
const H = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'fj_devtestadminkey000000',
  'Content-Type': 'application/json',
};
const INDEX = 'e2e-products';

setup('seed test data', async ({ request }) => {
  // 1. Backend must be running
  const health = await request.get(`${API}/health`);
  expect(health.ok(), 'Flapjack server must be running on port 7700').toBeTruthy();

  // 2. Clean slate — delete test index if it exists (ignore 404)
  await request.delete(`${API}/1/indexes/${INDEX}`, { headers: H }).catch(() => {});

  // 3. Add documents (creates index implicitly)
  const batchRes = await request.post(`${API}/1/indexes/${INDEX}/batch`, {
    headers: H,
    data: {
      requests: PRODUCTS.map((doc) => ({ action: 'addObject', body: doc })),
    },
  });
  expect(batchRes.ok(), 'Failed to batch-add documents').toBeTruthy();

  // 4. Configure settings
  const settingsRes = await request.put(`${API}/1/indexes/${INDEX}/settings`, {
    headers: H,
    data: SETTINGS,
  });
  expect(settingsRes.ok(), 'Failed to update settings').toBeTruthy();

  // 5. Add synonyms (batch)
  const synRes = await request.post(`${API}/1/indexes/${INDEX}/synonyms/batch`, {
    headers: H,
    data: SYNONYMS,
  });
  expect(synRes.ok(), 'Failed to batch-add synonyms').toBeTruthy();

  // 6. Add rules (batch)
  const rulesRes = await request.post(`${API}/1/indexes/${INDEX}/rules/batch`, {
    headers: H,
    data: RULES,
  });
  expect(rulesRes.ok(), 'Failed to batch-add rules').toBeTruthy();

  // 7. Seed analytics data (7 days of realistic search/click/geo data)
  await request.post(`${API}/2/analytics/seed`, {
    headers: H,
    data: { index: INDEX, days: 7 },
  });

  // 8. Wait for indexing to complete — poll until all documents are searchable
  await expect(async () => {
    const res = await request.post(`${API}/1/indexes/${INDEX}/query`, {
      headers: H,
      data: { query: '' },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body.nbHits).toBeGreaterThanOrEqual(PRODUCTS.length);
  }).toPass({ timeout: 15_000 });
});
