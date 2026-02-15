import { test, expect } from '../fixtures/auth.fixture';
import { seedAnalytics, deleteIndex, DEFAULT_ANALYTICS_CONFIG } from '../fixtures/analytics-seed';

/**
 * Analytics Data API Tests — PURE API (no browser)
 *
 * Seeds real analytics data, then verifies data integrity via REST API only.
 * These tests do NOT open a browser. For browser-based data verification,
 * see tests/e2e-ui/full/analytics-deep.spec.ts
 */

const API = 'http://localhost:7700';
const H = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'fj_devtestadminkey000000',
  'Content-Type': 'application/json',
};
const INDEX = 'test-analytics-api';

const EXPECTED = {
  totalSearches: DEFAULT_ANALYTICS_CONFIG.searchCount,
  uniqueUsers: 50,
  noResultRate: DEFAULT_ANALYTICS_CONFIG.noResultRate,
  desktopPct: DEFAULT_ANALYTICS_CONFIG.deviceDistribution.desktop,
  mobilePct: DEFAULT_ANALYTICS_CONFIG.deviceDistribution.mobile,
  tabletPct: DEFAULT_ANALYTICS_CONFIG.deviceDistribution.tablet,
  usPct: DEFAULT_ANALYTICS_CONFIG.countryDistribution.US,
  gbPct: DEFAULT_ANALYTICS_CONFIG.countryDistribution.GB,
};

async function skipIfNoServer({ request }: { request: any }) {
  try {
    const res = await request.get(`${API}/health`, { timeout: 3000 });
    if (!res.ok()) test.skip(true, 'Flapjack server not available');
  } catch {
    test.skip(true, 'Flapjack server not reachable');
  }
}

function getDateRange() {
  const now = new Date();
  const startDate = new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000).toISOString().split('T')[0];
  const endDate = now.toISOString().split('T')[0];
  return { startDate, endDate };
}

test.describe('Analytics Data API Verification (no browser)', () => {
  test.beforeAll(async ({ request }) => {
    await skipIfNoServer({ request });
    await seedAnalytics(request, {
      ...DEFAULT_ANALYTICS_CONFIG,
      indexName: INDEX,
    });
  });

  test.afterAll(async ({ request }) => {
    try { await deleteIndex(request, INDEX); } catch { /* ignore */ }
  });

  test.beforeEach(skipIfNoServer);

  test('search count returns positive total with daily breakdown', async ({ request }) => {
    const { startDate, endDate } = getDateRange();
    const res = await request.get(`${API}/2/searches/count`, {
      params: { index: INDEX, startDate, endDate },
      headers: H,
    });
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(data.count).toBeGreaterThanOrEqual(EXPECTED.totalSearches * 0.9);
    expect(data.dates).toBeDefined();
    expect(data.dates.length).toBeGreaterThan(0);
    const dailySum = data.dates.reduce((s: number, d: any) => s + d.count, 0);
    expect(dailySum).toBe(data.count);
    for (const d of data.dates) {
      expect(d.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(d.count).toBeGreaterThanOrEqual(0);
    }
  });

  test('unique users count is positive and less than total searches', async ({ request }) => {
    const { startDate, endDate } = getDateRange();
    const [usersRes, countRes] = await Promise.all([
      request.get(`${API}/2/users/count`, { params: { index: INDEX, startDate, endDate }, headers: H }),
      request.get(`${API}/2/searches/count`, { params: { index: INDEX, startDate, endDate }, headers: H }),
    ]);
    const users = await usersRes.json();
    const searches = await countRes.json();
    expect(users.count).toBeGreaterThanOrEqual(EXPECTED.uniqueUsers * 0.9);
    expect(users.count).toBeLessThan(searches.count);
  });

  test('no-result rate is between 0 and 1 with daily breakdown', async ({ request }) => {
    const { startDate, endDate } = getDateRange();
    const res = await request.get(`${API}/2/searches/noResultRate`, {
      params: { index: INDEX, startDate, endDate },
      headers: H,
    });
    const data = await res.json();
    expect(data.rate).toBeGreaterThanOrEqual(0);
    expect(data.rate).toBeLessThanOrEqual(1);
    expect(data.rate).toBeCloseTo(EXPECTED.noResultRate, 1);
    expect(data.dates).toBeDefined();
    for (const d of data.dates) {
      expect(d.rate).toBeGreaterThanOrEqual(0);
      expect(d.rate).toBeLessThanOrEqual(1);
    }
  });

  test('top searches are sorted by count descending', async ({ request }) => {
    const res = await request.get(`${API}/2/searches`, {
      params: { index: INDEX, ...getDateRange(), limit: '20' },
      headers: H,
    });
    const data = await res.json();
    expect(data.searches.length).toBeGreaterThan(0);
    for (let i = 1; i < data.searches.length; i++) {
      expect(data.searches[i].count).toBeLessThanOrEqual(data.searches[i - 1].count);
    }
    for (const s of data.searches) {
      expect(typeof s.search).toBe('string');
      expect(s.count).toBeGreaterThan(0);
      expect(typeof s.nbHits).toBe('number');
    }
  });

  test('no-results searches all have nbHits=0', async ({ request }) => {
    const res = await request.get(`${API}/2/searches/noResults`, {
      params: { index: INDEX, ...getDateRange(), limit: '50' },
      headers: H,
    });
    const data = await res.json();
    expect(data.searches.length).toBeGreaterThan(0);
    for (const s of data.searches) {
      expect(s.nbHits).toBe(0);
      expect(s.count).toBeGreaterThan(0);
    }
  });

  test('device breakdown sums to total searches', async ({ request }) => {
    const { startDate, endDate } = getDateRange();
    const [devicesRes, countRes] = await Promise.all([
      request.get(`${API}/2/devices`, { params: { index: INDEX, startDate, endDate }, headers: H }),
      request.get(`${API}/2/searches/count`, { params: { index: INDEX, startDate, endDate }, headers: H }),
    ]);
    const devices = await devicesRes.json();
    const searches = await countRes.json();
    const platforms = devices.platforms as any[];
    expect(platforms.length).toBeGreaterThanOrEqual(2);
    const platformSum = platforms.reduce((s: number, p: any) => s + p.count, 0);
    expect(platformSum).toBe(searches.count);
    const validPlatforms = ['desktop', 'mobile', 'tablet', 'unknown'];
    for (const p of platforms) {
      expect(validPlatforms).toContain(p.platform);
      expect(p.count).toBeGreaterThan(0);
    }
    const desktop = platforms.find((p: any) => p.platform === 'desktop');
    expect(desktop).toBeDefined();
    expect(desktop.count).toBeGreaterThan(platformSum * (EXPECTED.desktopPct - 0.1));
  });

  test('geo breakdown sums to total searches', async ({ request }) => {
    const { startDate, endDate } = getDateRange();
    const [geoRes, countRes] = await Promise.all([
      request.get(`${API}/2/geo`, { params: { index: INDEX, startDate, endDate }, headers: H }),
      request.get(`${API}/2/searches/count`, { params: { index: INDEX, startDate, endDate }, headers: H }),
    ]);
    const geo = await geoRes.json();
    const searches = await countRes.json();
    expect(geo.countries.length).toBeGreaterThanOrEqual(3);
    expect(geo.total).toBe(searches.count);
    const countrySum = geo.countries.reduce((s: number, c: any) => s + c.count, 0);
    expect(countrySum).toBe(geo.total);
    expect(geo.countries[0].country).toBe('US');
    for (const c of geo.countries) {
      expect(c.country).toMatch(/^[A-Z]{2}$/);
      expect(c.count).toBeGreaterThan(0);
    }
  });

  test('country filter produces subset of total searches', async ({ request }) => {
    const [allRes, usRes, gbRes] = await Promise.all([
      request.get(`${API}/2/searches`, { params: { index: INDEX, ...getDateRange(), limit: '5' }, headers: H }),
      request.get(`${API}/2/searches`, { params: { index: INDEX, ...getDateRange(), limit: '5', country: 'US' }, headers: H }),
      request.get(`${API}/2/searches`, { params: { index: INDEX, ...getDateRange(), limit: '5', country: 'GB' }, headers: H }),
    ]);
    const all = await allRes.json();
    const us = await usRes.json();
    const gb = await gbRes.json();
    expect(us.searches[0].count).toBeLessThan(all.searches[0].count);
    expect(gb.searches[0].count).toBeLessThan(all.searches[0].count);
    expect(us.searches[0].count).toBeGreaterThan(gb.searches[0].count);
  });

  test('device filter produces subset of total searches', async ({ request }) => {
    const [allRes, desktopRes] = await Promise.all([
      request.get(`${API}/2/searches`, { params: { index: INDEX, ...getDateRange(), limit: '5' }, headers: H }),
      request.get(`${API}/2/searches`, { params: { index: INDEX, ...getDateRange(), limit: '5', tags: 'platform:desktop' }, headers: H }),
    ]);
    const all = await allRes.json();
    const desktop = await desktopRes.json();
    expect(desktop.searches[0].count).toBeLessThan(all.searches[0].count);
    expect(desktop.searches[0].count).toBeGreaterThan(all.searches[0].count * 0.4);
  });

  test('geo top searches for a country returns results', async ({ request }) => {
    const res = await request.get(`${API}/2/geo/US`, {
      params: { index: INDEX, ...getDateRange() },
      headers: H,
    });
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(data.searches.length).toBeGreaterThan(0);
    for (const s of data.searches) {
      expect(typeof s.search).toBe('string');
      expect(s.count).toBeGreaterThan(0);
    }
  });
});
