import { test, expect } from '../fixtures/auth.fixture';
import { API_BASE, API_HEADERS } from '../fixtures/local-instance';

/**
 * Analytics API Shape Tests — PURE API (no browser)
 *
 * Verifies analytics REST API endpoints return correct shapes and status codes.
 * These tests do NOT open a browser. For browser-based analytics tests,
 * see tests/e2e-ui/full/analytics*.spec.ts
 */

async function skipIfNoServer({ request }: { request: any }) {
  try {
    const res = await request.get(`${API_BASE}/health`, { timeout: 3000 });
    if (!res.ok()) test.skip(true, 'Flapjack server not available');
  } catch {
    test.skip(true, 'Flapjack server not reachable');
  }
}

test.describe('Analytics API Response Shapes (no browser)', () => {
  test.beforeEach(skipIfNoServer);

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
    expect([200, 201, 202].includes(res.status())).toBeTruthy();
  });

  test('POST /1/events rejects events without required fields', async ({ request }) => {
    const res = await request.post(`${API_BASE}/1/events`, {
      headers: API_HEADERS,
      data: {
        events: [{
          index: 'test-index',
        }],
      },
    });
    expect([400, 422].includes(res.status())).toBeTruthy();
  });

  test('search with clickAnalytics=true returns queryID', async ({ request }) => {
    const indexName = `analytics-click-${Date.now()}`;

    const batchRes = await request.post(`${API_BASE}/1/indexes/${indexName}/batch`, {
      headers: API_HEADERS,
      data: {
        requests: [
          { action: 'addObject', body: { objectID: 'p1', name: 'Test Product', category: 'Electronics' } },
        ],
      },
    });
    expect(batchRes.ok()).toBeTruthy();

    await expect(async () => {
      const pollRes = await request.post(`${API_BASE}/1/indexes/${indexName}/query`, {
        headers: API_HEADERS,
        data: { query: '' },
      });
      expect(pollRes.ok()).toBeTruthy();
      const body = await pollRes.json();
      expect(body.nbHits).toBeGreaterThanOrEqual(1);
    }).toPass({ timeout: 15000 });

    const searchRes = await request.post(`${API_BASE}/1/indexes/${indexName}/query`, {
      headers: API_HEADERS,
      data: { query: 'test', clickAnalytics: true },
    });
    expect(searchRes.ok()).toBeTruthy();
    const body = await searchRes.json();
    expect(body).toHaveProperty('queryID');
    expect(typeof body.queryID).toBe('string');
    expect(body.queryID.length).toBe(32);

    await request.delete(`${API_BASE}/1/indexes/${indexName}`, { headers: API_HEADERS });
  });
});
