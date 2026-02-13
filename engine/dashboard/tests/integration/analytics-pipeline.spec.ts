import { test, expect } from '../fixtures/auth.fixture';

/**
 * Analytics Pipeline E2E Tests
 *
 * Tests the full flow: search -> analytics data collection -> dashboard display.
 *
 * These tests verify that:
 * 1. API endpoints respond with expected status codes and shapes
 * 2. The analytics page loads and renders correctly
 * 3. Tab switching works and shows appropriate content
 * 4. Date range changes trigger new API calls
 *
 * Prerequisites:
 * - Flapjack server running on localhost:7700
 */

const API_BASE = 'http://localhost:7700';
const API_HEADERS = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'abcdef0123456789',
  'Content-Type': 'application/json',
};

async function skipIfNoServer({ request }: { request: any }) {
  try {
    const res = await request.get(`${API_BASE}/health`, { timeout: 3000 });
    if (!res.ok()) test.skip(true, 'Flapjack server not available');
  } catch {
    test.skip(true, 'Flapjack server not reachable');
  }
}

test.describe('Analytics Pipeline E2E', () => {
  test.describe('Backend API Response Shape', () => {
    test.beforeEach(skipIfNoServer);
    // These endpoints return 200 with data. The server always returns a valid
    // shape for analytics endpoints, even when the index has no data.
    test('GET /2/searches/count returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/searches/count`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('count');
      expect(typeof body.count).toBe('number');
      if (body.dates) {
        expect(Array.isArray(body.dates)).toBe(true);
      }
    });

    test('GET /2/searches returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/searches`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31', limit: '5' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('searches');
      expect(Array.isArray(body.searches)).toBe(true);
    });

    test('GET /2/searches/noResultRate returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/searches/noResultRate`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('rate');
      expect(typeof body.rate).toBe('number');
    });

    test('GET /2/clicks/clickThroughRate returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/clicks/clickThroughRate`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('rate');
      expect(body).toHaveProperty('clickCount');
      expect(body).toHaveProperty('trackedSearchCount');
    });

    test('GET /2/users/count returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/users/count`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('count');
      expect(typeof body.count).toBe('number');
    });

    test('GET /2/filters returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/filters`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('filters');
      expect(Array.isArray(body.filters)).toBe(true);
    });

    test('GET /2/hits returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/hits`, {
        params: { index: 'test-index', startDate: '2026-01-01', endDate: '2026-12-31' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('hits');
      expect(Array.isArray(body.hits)).toBe(true);
    });

    test('GET /2/status returns expected shape', async ({ request }) => {
      const res = await request.get(`${API_BASE}/2/status`, {
        params: { index: 'movies' },
        headers: API_HEADERS,
      });

      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(body).toHaveProperty('enabled');
      expect(body).toHaveProperty('hasData');
      expect(body).toHaveProperty('retentionDays');
      expect(typeof body.enabled).toBe('boolean');
      expect(typeof body.hasData).toBe('boolean');
      expect(typeof body.retentionDays).toBe('number');
    });

    test('POST /1/events accepts valid insight events', async ({ request }) => {
      const res = await request.post(`${API_BASE}/1/events`, {
        headers: API_HEADERS,
        data: {
          events: [{
            eventType: 'click',
            eventName: 'Test Click',
            index: 'test-index',
            userToken: 'test-user-123',
            objectIDs: ['obj-1'],
            positions: [1],
            timestamp: Date.now(),
          }],
        },
      });

      // Should accept even if analytics is not fully initialized (202/200)
      expect([200, 201, 202].includes(res.status())).toBeTruthy();
    });

    test('POST /1/events rejects events without required fields', async ({ request }) => {
      const res = await request.post(`${API_BASE}/1/events`, {
        headers: API_HEADERS,
        data: {
          events: [{
            // Missing required fields like eventType, eventName, userToken
            index: 'test-index',
          }],
        },
      });

      // Should reject with 4xx
      expect([400, 422].includes(res.status())).toBeTruthy();
    });
  });

  test.describe('Full Pipeline: Search -> Analytics -> Dashboard', () => {
    test.beforeEach(skipIfNoServer);

    test('search with clickAnalytics=true returns queryID', async ({ request }) => {
      // Use a unique index name to avoid stale state from previous runs
      const indexName = `analytics-click-${Date.now()}`;

      // Add a document (auto-creates the index)
      const batchRes = await request.post(`${API_BASE}/1/indexes/${indexName}/batch`, {
        headers: API_HEADERS,
        data: {
          requests: [
            { action: 'addObject', body: { objectID: 'p1', name: 'Test Product', category: 'Electronics' } },
          ],
        },
      });
      expect(batchRes.ok()).toBeTruthy();

      // Poll until the document is indexed and searchable
      await expect(async () => {
        const pollRes = await request.post(`${API_BASE}/1/indexes/${indexName}/query`, {
          headers: API_HEADERS,
          data: { query: '' },
        });
        expect(pollRes.ok()).toBeTruthy();
        const body = await pollRes.json();
        expect(body.nbHits).toBeGreaterThanOrEqual(1);
      }).toPass({ timeout: 15000 });

      // Search with clickAnalytics enabled
      const searchRes = await request.post(`${API_BASE}/1/indexes/${indexName}/query`, {
        headers: API_HEADERS,
        data: { query: 'test', clickAnalytics: true },
      });

      expect(searchRes.ok()).toBeTruthy();
      const body = await searchRes.json();
      // When clickAnalytics is true, response should include queryID
      expect(body).toHaveProperty('queryID');
      expect(typeof body.queryID).toBe('string');
      expect(body.queryID.length).toBe(32); // 32-char hex

      // Cleanup
      await request.delete(`${API_BASE}/1/indexes/${indexName}`, {
        headers: API_HEADERS,
      });
    });

    test('dashboard analytics page loads heading and KPI cards', async ({ page }) => {
      await page.goto('/index/analytics-e2e-test/analytics');

      // Heading must be visible
      await expect(page.getByTestId('analytics-heading')).toBeVisible();

      // KPI cards must always render (even with zero values)
      await expect(page.getByTestId('kpi-cards')).toBeVisible();
    });

    test('switching date ranges triggers new API calls', async ({ page }) => {
      await page.goto('/index/analytics-e2e-test/analytics');
      await expect(page.getByTestId('analytics-heading')).toBeVisible();

      // Intercept analytics API calls
      const apiCalls: string[] = [];
      await page.route('**/2/**', (route) => {
        apiCalls.push(route.request().url());
        route.continue();
      });

      // Click 30d range button
      const btn30d = page.getByTestId('range-30d');
      await btn30d.click();

      // Wait for analytics API call — this MUST fire when switching ranges
      await page.waitForResponse(
        resp => resp.url().includes('/2/'),
        { timeout: 5000 },
      );

      // Verify at least one call was made with date parameters
      expect(apiCalls.length).toBeGreaterThan(0);
      const hasDateParams = apiCalls.some(url =>
        url.includes('startDate=') && url.includes('endDate='),
      );
      expect(hasDateParams).toBeTruthy();
    });

    test('tab navigation loads tab-specific content', async ({ page }) => {
      await page.goto('/index/analytics-e2e-test/analytics');
      await expect(page.getByTestId('analytics-heading')).toBeVisible();

      // Test tabs that exist in the UI. Each tab must load its expected content.
      const tabs = [
        { trigger: 'tab-searches', content: ['top-searches-table', 'searches-filter'] },
        { trigger: 'tab-no-results', content: ['no-results-table', 'no-result-rate-banner'] },
        { trigger: 'tab-filters', content: ['filters-table'] },
      ];

      let tabsTested = 0;
      for (const tab of tabs) {
        const trigger = page.getByTestId(tab.trigger);
        const isVisible = await trigger.isVisible().catch(() => false);
        if (!isVisible) continue;

        await trigger.click();
        tabsTested++;

        // Wait for at least one of the expected content elements
        const contentLocators = tab.content.map(id => page.getByTestId(id));
        const anyContent = contentLocators.reduce(
          (acc, loc) => acc.or(loc),
        );
        await expect(anyContent.first()).toBeVisible({ timeout: 5000 });
      }

      // At least 2 tabs must be testable — if none exist, something is very wrong
      expect(tabsTested).toBeGreaterThanOrEqual(2);
    });
  });
});
