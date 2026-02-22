/**
 * Playwright setup project — seeds test data into the real Flapjack backend.
 * Runs ONCE before any e2e-ui tests.
 *
 * Requires: Flapjack server running on the repo-local configured backend port.
 */
import { test as setup, expect } from '@playwright/test';
import { PRODUCTS, SYNONYMS, RULES, SETTINGS } from '../fixtures/test-data';
import { API_BASE as API, API_HEADERS as H } from '../fixtures/local-instance';
import { deleteExperimentsByName } from '../fixtures/api-helpers';

const INDEX = 'e2e-products';

setup('seed test data', async ({ request }) => {
  // 1. Backend must be running
  const health = await request.get(`${API}/health`);
  expect(
    health.ok(),
    `Flapjack server must be running at ${API}`,
  ).toBeTruthy();

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

  // 9. Seed a baseline experiment for experiment browser-unmocked tests.
  await deleteExperimentsByName(request, 'e2e-seeded-experiment');

  const expRes = await request.post(`${API}/2/abtests`, {
    headers: H,
    data: {
      name: 'e2e-seeded-experiment',
      indexName: INDEX,
      trafficSplit: 0.5,
      control: { name: 'control' },
      variant: {
        name: 'variant',
        queryOverrides: { filters: 'brand:Apple' },
      },
      primaryMetric: 'ctr',
      minimumDays: 14,
    },
  });
  expect(expRes.ok(), 'Failed to create seeded experiment').toBeTruthy();
  const seededExp = await expRes.json();

  // Start the experiment so it has a running status for detail page tests
  const startRes = await request.post(`${API}/2/abtests/${seededExp.id}/start`, { headers: H });
  expect(startRes.ok(), 'Failed to start seeded experiment').toBeTruthy();
});
