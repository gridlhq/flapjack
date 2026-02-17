import { test, expect } from '../fixtures/auth.fixture';

/**
 * Demo Analytics API Tests — PURE API (no browser)
 *
 * Tests the analytics seed/flush/clear API endpoints directly.
 * These tests do NOT open a browser. For browser-based demo analytics verification,
 * see tests/e2e-ui/full/analytics*.spec.ts
 */

const API_BASE = 'http://localhost:7700';
const API_HEADERS = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'fj_devtestadminkey000000',
  'Content-Type': 'application/json',
};

test.describe('Analytics Management API (no browser)', () => {
  test('flush endpoint triggers immediate analytics update', async ({ request }) => {
    const res = await request.post(`${API_BASE}/2/analytics/flush`, {
      headers: API_HEADERS,
    });
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(data.status).toBe('ok');
  });

  test('clear endpoint removes all analytics for an index', async ({ request }) => {
    const clearIndex = `clear-test-${Date.now()}`;
    await request.post(`${API_BASE}/2/analytics/seed`, {
      headers: API_HEADERS,
      data: { index: clearIndex, days: 7 },
    });

    const clearRes = await request.delete(`${API_BASE}/2/analytics/clear`, {
      headers: API_HEADERS,
      data: { index: clearIndex },
    });
    expect(clearRes.ok()).toBeTruthy();
    const clearData = await clearRes.json();
    expect(clearData.status).toBe('ok');
    expect(clearData.partitionsRemoved).toBeGreaterThan(0);

    const countRes = await request.get(`${API_BASE}/2/searches/count`, {
      params: { index: clearIndex, startDate: '2025-01-01', endDate: '2027-01-01' },
      headers: API_HEADERS,
    });
    if (countRes.ok()) {
      const countData = await countRes.json();
      expect(countData.count).toBe(0);
    }
  });

  test('seed endpoint generates analytics data', async ({ request }) => {
    const INDEX_NAME = `seed-api-test-${Date.now()}`;
    const seedRes = await request.post(`${API_BASE}/2/analytics/seed`, {
      headers: API_HEADERS,
      data: { index: INDEX_NAME, days: 30 },
    });
    expect(seedRes.ok()).toBeTruthy();
    const seedData = await seedRes.json();
    expect(seedData.totalSearches).toBeGreaterThan(0);
    expect(seedData.totalClicks).toBeGreaterThan(0);

    const countRes = await request.get(`${API_BASE}/2/searches/count`, {
      params: { index: INDEX_NAME, startDate: '2025-01-01', endDate: '2027-01-01' },
      headers: API_HEADERS,
    });
    expect(countRes.ok()).toBeTruthy();
    const countData = await countRes.json();
    expect(countData.count).toBeGreaterThan(0);

    // Cleanup
    await request.delete(`${API_BASE}/1/indexes/${INDEX_NAME}`, { headers: API_HEADERS });
    await request.delete(`${API_BASE}/2/analytics/clear`, {
      headers: API_HEADERS,
      data: { index: INDEX_NAME },
    });
  });
});
