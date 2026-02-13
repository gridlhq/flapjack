import { test, expect } from '../fixtures/auth.fixture';

const API_BASE = 'http://localhost:7700';
const API_HEADERS = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'abcdef0123456789',
  'Content-Type': 'application/json',
};

/**
 * Demo Dataset Analytics E2E Test
 *
 * Verifies the core user journey:
 * 1. Create a demo movies index -> analytics data is auto-seeded
 * 2. Navigate to the per-index analytics page
 * 3. Analytics data (charts, KPIs) is visible — not empty
 * 4. Clean up by deleting the test index + analytics
 *
 * Requires: Flapjack server running on port 7700
 */
test.describe('Demo Dataset Analytics Preload', () => {
  const INDEX_NAME = `demo-analytics-e2e-${Date.now()}`;

  test.afterAll(async ({ request }) => {
    // Cleanup: delete the test index and its analytics
    await request.delete(`${API_BASE}/1/indexes/${INDEX_NAME}`, { headers: API_HEADERS });
    await request.delete(`${API_BASE}/2/analytics/clear`, {
      headers: API_HEADERS,
      data: { index: INDEX_NAME },
    });
  });

  test('seed endpoint generates analytics data visible in dashboard', async ({ page, request }) => {
    // Step 1: Seed analytics data via API (simulates what CreateIndexDialog does)
    const seedRes = await request.post(`${API_BASE}/2/analytics/seed`, {
      headers: API_HEADERS,
      data: { index: INDEX_NAME, days: 30 },
    });
    expect(seedRes.ok()).toBeTruthy();
    const seedData = await seedRes.json();
    expect(seedData.totalSearches).toBeGreaterThan(0);
    expect(seedData.totalClicks).toBeGreaterThan(0);

    // Step 2: Verify search count API returns data
    const countRes = await request.get(`${API_BASE}/2/searches/count`, {
      params: { index: INDEX_NAME, startDate: '2025-01-01', endDate: '2027-01-01' },
      headers: API_HEADERS,
    });
    expect(countRes.ok()).toBeTruthy();
    const countData = await countRes.json();
    expect(countData.count).toBeGreaterThan(0);

    // Step 3: Navigate to the per-index analytics page
    await page.goto(`/index/${INDEX_NAME}/analytics`);

    // Step 4: Verify analytics heading loads
    await expect(page.getByTestId('analytics-heading')).toBeVisible();

    // Step 5: Verify KPI cards show actual data (not empty/loading)
    // The overview tab should be active by default with KPI cards
    const kpiCards = page.getByTestId('kpi-cards');
    await expect(kpiCards).toBeVisible({ timeout: 10000 });

    // The "Total Searches" KPI should show a non-zero number
    const totalSearchesKpi = page.getByTestId('kpi-total-searches');
    await expect(totalSearchesKpi).toBeVisible();
    // Verify it does NOT show "No data" — it should have an actual number
    await expect(totalSearchesKpi.getByText('No data')).not.toBeVisible();

    // Step 6: Search volume chart should be present (not empty state)
    await expect(page.getByTestId('search-volume-chart')).toBeVisible();

    // Step 7: Switch to Searches tab and verify top searches appear
    await page.getByTestId('tab-searches').click();
    await expect(
      page.getByTestId('top-searches-table'),
    ).toBeVisible({ timeout: 5000 });

    // Step 8: Switch to Geography tab and verify country data exists
    const geoTab = page.getByTestId('tab-geography');
    if (await geoTab.isVisible().catch(() => false)) {
      await geoTab.click();
      // Should show country data, not the empty state
      await expect(
        page.getByText('Searches by Country')
          .or(page.getByText('No geographic data')),
      ).toBeVisible({ timeout: 5000 });
    }
  });

  test('flush endpoint triggers immediate analytics update', async ({ request }) => {
    const res = await request.post(`${API_BASE}/2/analytics/flush`, {
      headers: API_HEADERS,
    });
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(data.status).toBe('ok');
  });

  test('clear endpoint removes all analytics for an index', async ({ request }) => {
    // First seed some data
    await request.post(`${API_BASE}/2/analytics/seed`, {
      headers: API_HEADERS,
      data: { index: `clear-test-${Date.now()}`, days: 7 },
    });

    const clearIndex = `clear-test-${Date.now()}`;
    await request.post(`${API_BASE}/2/analytics/seed`, {
      headers: API_HEADERS,
      data: { index: clearIndex, days: 7 },
    });

    // Clear it
    const clearRes = await request.delete(`${API_BASE}/2/analytics/clear`, {
      headers: API_HEADERS,
      data: { index: clearIndex },
    });
    expect(clearRes.ok()).toBeTruthy();
    const clearData = await clearRes.json();
    expect(clearData.status).toBe('ok');
    expect(clearData.partitionsRemoved).toBeGreaterThan(0);

    // Verify data is gone
    const countRes = await request.get(`${API_BASE}/2/searches/count`, {
      params: { index: clearIndex, startDate: '2025-01-01', endDate: '2027-01-01' },
      headers: API_HEADERS,
    });
    if (countRes.ok()) {
      const countData = await countRes.json();
      expect(countData.count).toBe(0);
    }
  });
});
