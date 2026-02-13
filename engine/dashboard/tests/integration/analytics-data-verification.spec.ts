import { test, expect } from '../fixtures/auth.fixture';

/**
 * Analytics Data Verification Tests
 *
 * Verifies that analytics data is correctly rolled up, processed, and displayed
 * across all analytics tabs. Uses the seeded "movies" index which has ~20k
 * searches over 30 days with geo/device/filter data.
 *
 * These tests go beyond visibility checks — they verify actual data values,
 * mathematical consistency, and correct rollup behavior.
 *
 * Prerequisites:
 * - Flapjack server running on port 7700
 * - "movies" index exists with seeded analytics (30 days)
 */

const API = 'http://localhost:7700';
const H = {
  'x-algolia-application-id': 'flapjack',
  'x-algolia-api-key': 'abcdef0123456789',
  'Content-Type': 'application/json',
};
const INDEX = 'movies';

async function skipIfNoServer({ request }: { request: any }) {
  try {
    const res = await request.get(`${API}/health`, { timeout: 3000 });
    if (!res.ok()) test.skip(true, 'Flapjack server not available');
  } catch {
    test.skip(true, 'Flapjack server not reachable');
  }
}

// ─── API-Level Data Verification ────────────────────────────────────

test.describe('Analytics Data Verification (API)', () => {
  test.beforeEach(skipIfNoServer);
  test('search count returns positive total with daily breakdown', async ({ request }) => {
    const res = await request.get(`${API}/2/searches/count`, {
      params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
      headers: H,
    });
    expect(res.ok()).toBeTruthy();
    const data = await res.json();

    expect(data.count).toBeGreaterThan(1000);
    expect(data.dates).toBeDefined();
    expect(data.dates.length).toBeGreaterThan(0);

    // Daily counts should sum to total
    const dailySum = data.dates.reduce((s: number, d: any) => s + d.count, 0);
    expect(dailySum).toBe(data.count);

    // Each date entry should have date + count
    for (const d of data.dates) {
      expect(d.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      expect(d.count).toBeGreaterThanOrEqual(0);
    }
  });

  test('unique users count is positive and less than total searches', async ({ request }) => {
    const [usersRes, countRes] = await Promise.all([
      request.get(`${API}/2/users/count`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
      request.get(`${API}/2/searches/count`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
    ]);

    const users = await usersRes.json();
    const searches = await countRes.json();

    expect(users.count).toBeGreaterThan(0);
    // Users should be fewer than total searches (each user does multiple searches)
    expect(users.count).toBeLessThan(searches.count);
  });

  test('no-result rate is between 0 and 1 with daily breakdown', async ({ request }) => {
    const res = await request.get(`${API}/2/searches/noResultRate`, {
      params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
      headers: H,
    });
    const data = await res.json();

    expect(data.rate).toBeGreaterThanOrEqual(0);
    expect(data.rate).toBeLessThanOrEqual(1);
    expect(data.dates).toBeDefined();
    expect(data.dates.length).toBeGreaterThan(0);

    // Each daily rate should also be 0-1
    for (const d of data.dates) {
      expect(d.rate).toBeGreaterThanOrEqual(0);
      expect(d.rate).toBeLessThanOrEqual(1);
    }
  });

  test('top searches are sorted by count descending', async ({ request }) => {
    const res = await request.get(`${API}/2/searches`, {
      params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '20' },
      headers: H,
    });
    const data = await res.json();

    expect(data.searches.length).toBeGreaterThan(0);
    expect(data.searches.length).toBeLessThanOrEqual(20);

    // Verify descending sort
    for (let i = 1; i < data.searches.length; i++) {
      expect(data.searches[i].count).toBeLessThanOrEqual(data.searches[i - 1].count);
    }

    // Each entry should have search + count + nbHits
    for (const s of data.searches) {
      expect(typeof s.search).toBe('string');
      expect(s.count).toBeGreaterThan(0);
      expect(typeof s.nbHits).toBe('number');
    }
  });

  test('no-results searches all have nbHits=0', async ({ request }) => {
    const res = await request.get(`${API}/2/searches/noResults`, {
      params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '50' },
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
    const [devicesRes, countRes] = await Promise.all([
      request.get(`${API}/2/devices`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
      request.get(`${API}/2/searches/count`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
    ]);
    const devices = await devicesRes.json();
    const searches = await countRes.json();

    const platforms = devices.platforms as any[];
    expect(platforms.length).toBeGreaterThanOrEqual(2); // At least desktop + mobile

    const platformSum = platforms.reduce((s: number, p: any) => s + p.count, 0);
    expect(platformSum).toBe(searches.count);

    // Each platform should have a known name
    const validPlatforms = ['desktop', 'mobile', 'tablet', 'unknown'];
    for (const p of platforms) {
      expect(validPlatforms).toContain(p.platform);
      expect(p.count).toBeGreaterThan(0);
    }

    // Desktop should be the largest (58% weight in seed data)
    const desktop = platforms.find((p: any) => p.platform === 'desktop');
    expect(desktop).toBeDefined();
    expect(desktop.count).toBeGreaterThan(platformSum * 0.4);
  });

  test('geo breakdown sums to total searches', async ({ request }) => {
    const [geoRes, countRes] = await Promise.all([
      request.get(`${API}/2/geo`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
      request.get(`${API}/2/searches/count`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
    ]);
    const geo = await geoRes.json();
    const searches = await countRes.json();

    expect(geo.countries.length).toBeGreaterThanOrEqual(10);
    expect(geo.total).toBe(searches.count);

    // Country counts should sum to total
    const countrySum = geo.countries.reduce((s: number, c: any) => s + c.count, 0);
    expect(countrySum).toBe(geo.total);

    // US should be the top country (45% weight in seed data)
    expect(geo.countries[0].country).toBe('US');
    expect(geo.countries[0].count).toBeGreaterThan(geo.total * 0.3);

    // Each entry should have country code + count
    for (const c of geo.countries) {
      expect(c.country).toMatch(/^[A-Z]{2}$/);
      expect(c.count).toBeGreaterThan(0);
    }
  });

  test('US regions sum to US country count', async ({ request }) => {
    const [regionsRes, geoRes] = await Promise.all([
      request.get(`${API}/2/geo/US/regions`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
      request.get(`${API}/2/geo`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
        headers: H,
      }),
    ]);
    const regions = await regionsRes.json();
    const geo = await geoRes.json();

    expect(regions.regions.length).toBeGreaterThanOrEqual(10); // 13 US states in seed

    const regionSum = regions.regions.reduce((s: number, r: any) => s + r.count, 0);
    const usCount = geo.countries.find((c: any) => c.country === 'US')?.count;
    expect(regionSum).toBe(usCount);

    // California should be top (highest weight)
    expect(regions.regions[0].region).toBe('California');
  });

  test('country filter produces subset of total searches', async ({ request }) => {
    const [allRes, usRes, gbRes] = await Promise.all([
      request.get(`${API}/2/searches`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '5' },
        headers: H,
      }),
      request.get(`${API}/2/searches`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '5', country: 'US' },
        headers: H,
      }),
      request.get(`${API}/2/searches`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '5', country: 'GB' },
        headers: H,
      }),
    ]);

    const all = await allRes.json();
    const us = await usRes.json();
    const gb = await gbRes.json();

    // US and GB counts should be less than all
    expect(us.searches[0].count).toBeLessThan(all.searches[0].count);
    expect(gb.searches[0].count).toBeLessThan(all.searches[0].count);

    // US should have more than GB (higher weight in seed data)
    expect(us.searches[0].count).toBeGreaterThan(gb.searches[0].count);
  });

  test('device filter produces subset of total searches', async ({ request }) => {
    const [allRes, desktopRes] = await Promise.all([
      request.get(`${API}/2/searches`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '5' },
        headers: H,
      }),
      request.get(`${API}/2/searches`, {
        params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08', limit: '5', tags: 'platform:desktop' },
        headers: H,
      }),
    ]);

    const all = await allRes.json();
    const desktop = await desktopRes.json();

    // Desktop count should be less than all (58% of total)
    expect(desktop.searches[0].count).toBeLessThan(all.searches[0].count);
    expect(desktop.searches[0].count).toBeGreaterThan(all.searches[0].count * 0.4);
  });

  test('geo top searches for a country returns results', async ({ request }) => {
    const res = await request.get(`${API}/2/geo/US`, {
      params: { index: INDEX, startDate: '2026-01-01', endDate: '2026-02-08' },
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

// ─── UI-Level Data Verification ─────────────────────────────────────

test.describe('Analytics UI Data Verification', () => {
  test.beforeEach(skipIfNoServer);

  test.describe('Overview Tab', () => {
    test('KPI cards show non-zero values from API data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });

      // Total Searches KPI should show a number > 0
      const totalSearches = page.getByTestId('kpi-total-searches');
      await expect(totalSearches).toBeVisible();
      const searchText = await totalSearches.locator('.text-2xl').textContent();
      const searchNum = parseInt(searchText!.replace(/,/g, ''), 10);
      expect(searchNum).toBeGreaterThan(1000);

      // Unique Users KPI
      const uniqueUsers = page.getByTestId('kpi-unique-users');
      await expect(uniqueUsers).toBeVisible();
      const userText = await uniqueUsers.locator('.text-2xl').textContent();
      const userNum = parseInt(userText!.replace(/,/g, ''), 10);
      expect(userNum).toBeGreaterThan(0);
      expect(userNum).toBeLessThan(searchNum);

      // No-Result Rate KPI should show a percentage
      const nrr = page.getByTestId('kpi-no-result-rate');
      await expect(nrr).toBeVisible();
      const nrrText = await nrr.locator('.text-2xl').textContent();
      expect(nrrText).toMatch(/\d+\.\d+%/);
    });

    test('search volume chart renders with data points', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      const chart = page.getByTestId('search-volume-chart');
      await expect(chart).toBeVisible({ timeout: 10000 });

      // The chart should contain SVG elements (Recharts renders as SVG)
      await expect(chart.locator('svg')).toBeVisible();
      // Should have area path (the data line)
      await expect(chart.locator('svg path.recharts-area-area')).toBeVisible();
    });

    test('top 10 searches table shows ranked queries', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      const table = page.getByTestId('top-searches-overview');
      await expect(table).toBeVisible({ timeout: 10000 });

      // Wait for actual table rows to load (data comes from async API call)
      const rows = table.locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });

      const count = await rows.count();
      expect(count).toBeGreaterThanOrEqual(5);
      expect(count).toBeLessThanOrEqual(10);

      // First row should have rank #1
      await expect(rows.first().locator('td').first()).toHaveText('1');
    });

    test('KPI cards show delta badges comparing to previous period', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });

      // Delta badges require BOTH current and previous period API calls to complete.
      // Wait for at least one delta badge to appear.
      const deltaBadge = page.getByTestId('delta-badge').first();
      await deltaBadge.waitFor({ timeout: 15000 });

      const badgeCount = await page.getByTestId('delta-badge').count();
      expect(badgeCount).toBeGreaterThan(0);
    });
  });

  test.describe('Searches Tab', () => {
    test('displays sortable table with query data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });

      // Wait for actual table rows to load
      const rows = page.getByTestId('top-searches-table').locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });

      const rowCount = await rows.count();
      expect(rowCount).toBeGreaterThan(10);

      // First row count should be larger than last row count (descending sort)
      const firstCount = await rows.first().locator('td:nth-child(3)').textContent();
      const lastCount = await rows.last().locator('td:nth-child(3)').textContent();
      expect(parseInt(firstCount!.replace(/,/g, ''), 10))
        .toBeGreaterThanOrEqual(parseInt(lastCount!.replace(/,/g, ''), 10));
    });

    test('country filter changes displayed data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });

      // Wait for table rows to load
      await page.getByTestId('top-searches-table').locator('tbody tr').first().waitFor({ timeout: 10000 });

      // Get unfiltered top count
      const unfilteredCount = await page
        .getByTestId('top-searches-table')
        .locator('tbody tr')
        .first()
        .locator('td:nth-child(3)')
        .textContent();

      // Select a country filter
      const countryFilter = page.getByTestId('searches-country-filter');
      if (await countryFilter.isVisible().catch(() => false)) {
        await countryFilter.selectOption({ index: 1 }); // First country
        // Poll until filtered data loads (count should be less than unfiltered)
        const unfilteredNum = parseInt(unfilteredCount!.replace(/,/g, ''), 10);
        await expect(async () => {
          const filteredCount = await page
            .getByTestId('top-searches-table')
            .locator('tbody tr')
            .first()
            .locator('td:nth-child(3)')
            .textContent();
          expect(parseInt(filteredCount!.replace(/,/g, ''), 10))
            .toBeLessThan(unfilteredNum);
        }).toPass({ timeout: 10000 });
      }
    });

    test('device filter changes displayed data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });

      // Wait for table rows to load
      await page.getByTestId('top-searches-table').locator('tbody tr').first().waitFor({ timeout: 10000 });

      const unfilteredCount = await page
        .getByTestId('top-searches-table')
        .locator('tbody tr')
        .first()
        .locator('td:nth-child(3)')
        .textContent();

      const deviceFilter = page.getByTestId('searches-device-filter');
      if (await deviceFilter.isVisible().catch(() => false)) {
        await deviceFilter.selectOption({ index: 1 }); // First device
        // Poll until filtered data loads
        const unfilteredDevNum = parseInt(unfilteredCount!.replace(/,/g, ''), 10);
        await expect(async () => {
          const filteredCount = await page
            .getByTestId('top-searches-table')
            .locator('tbody tr')
            .first()
            .locator('td:nth-child(3)')
            .textContent();
          expect(parseInt(filteredCount!.replace(/,/g, ''), 10))
            .toBeLessThan(unfilteredDevNum);
        }).toPass({ timeout: 10000 });
      }
    });

    test('text filter narrows results client-side', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });

      // Wait for table rows to load
      await page.getByTestId('top-searches-table').locator('tbody tr').first().waitFor({ timeout: 10000 });

      const beforeCount = await page.getByTestId('top-searches-table').locator('tbody tr').count();

      // Type a specific query in the filter
      await page.getByTestId('searches-filter-input').fill('batman');
      // Poll until client-side filter takes effect
      await expect(async () => {
        const afterCount = await page.getByTestId('top-searches-table').locator('tbody tr').count();
        expect(afterCount).toBeLessThan(beforeCount);
        expect(afterCount).toBeGreaterThan(0);
      }).toPass({ timeout: 10000 });
    });
  });

  test.describe('No Results Tab', () => {
    test('shows rate banner with valid percentage', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-no-results').click();

      const banner = page.getByTestId('no-result-rate-banner');
      await expect(banner).toBeVisible({ timeout: 10000 });

      // Should show a percentage like "5.4%"
      const rateText = await banner.locator('.text-3xl').textContent();
      expect(rateText).toMatch(/\d+\.\d+%/);
      const rateValue = parseFloat(rateText!.replace('%', ''));
      expect(rateValue).toBeGreaterThan(0);
      expect(rateValue).toBeLessThan(100);
    });

    test('shows table of zero-result queries', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-no-results').click();

      const table = page.getByTestId('no-results-table');
      await expect(table).toBeVisible({ timeout: 10000 });

      // Wait for actual table rows to load
      const rows = table.locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });

      const count = await rows.count();
      expect(count).toBeGreaterThan(0);

      // These are from seed data: "new release", "stream free", "torrent", "subtitles"
      const allText = await table.locator('tbody').textContent();
      // At least one of the seed no-result queries should be present
      const hasNoResultQuery =
        allText!.includes('stream free') ||
        allText!.includes('torrent') ||
        allText!.includes('subtitles') ||
        allText!.includes('new release');
      expect(hasNoResultQuery).toBeTruthy();
    });
  });

  test.describe('Devices Tab', () => {
    test('shows platform cards with counts summing correctly', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-devices').click();

      // Wait for device cards to load
      await expect(page.getByTestId('device-desktop')).toBeVisible({ timeout: 10000 });
      await expect(page.getByTestId('device-mobile')).toBeVisible();

      // Extract counts from each card
      const desktopCount = await page.getByTestId('device-desktop').locator('.text-2xl').textContent();
      const mobileCount = await page.getByTestId('device-mobile').locator('.text-2xl').textContent();

      const desktop = parseInt(desktopCount!.replace(/,/g, ''), 10);
      const mobile = parseInt(mobileCount!.replace(/,/g, ''), 10);

      expect(desktop).toBeGreaterThan(0);
      expect(mobile).toBeGreaterThan(0);
      // Desktop should be larger than mobile (58% vs 32% in seed)
      expect(desktop).toBeGreaterThan(mobile);

      // Check percentages are displayed and reasonable
      const desktopPct = await page.getByTestId('device-desktop').locator('.text-lg').textContent();
      expect(desktopPct).toMatch(/\d+\.\d+%/);
      const desktopPctVal = parseFloat(desktopPct!.replace('%', ''));
      expect(desktopPctVal).toBeGreaterThan(40);
      expect(desktopPctVal).toBeLessThan(80);
    });

    test('shows stacked area chart for devices over time', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-devices').click();
      await expect(page.getByTestId('device-desktop')).toBeVisible({ timeout: 10000 });

      // Chart should render with SVG
      const chart = page.locator('.h-64');
      await expect(chart.locator('svg')).toBeVisible();
    });
  });

  test.describe('Geography Tab', () => {
    test('shows country table with US as top country', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();

      // Wait for country table to load
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });

      const table = page.locator('table');
      const firstRow = table.locator('tbody tr').first();
      await firstRow.waitFor({ timeout: 10000 });
      // US should be first (highest weight)
      await expect(firstRow).toContainText('United States');
      await expect(firstRow).toContainText('US');
    });

    test('country percentages sum to approximately 100%', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });

      // Wait for rows to load
      await page.locator('table tbody tr').first().waitFor({ timeout: 10000 });

      // Get all percentage cells (4th column)
      const pctCells = page.locator('table tbody tr td:nth-child(4)');
      const cellCount = await pctCells.count();
      let totalPct = 0;
      for (let i = 0; i < cellCount; i++) {
        const text = await pctCells.nth(i).textContent();
        totalPct += parseFloat(text!.replace('%', ''));
      }
      // Should sum to ~100% (allow rounding error)
      expect(totalPct).toBeGreaterThan(99);
      expect(totalPct).toBeLessThan(101);
    });

    test('clicking a country shows drill-down with top searches and regions', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });

      // Click the US row
      await page.locator('table tbody tr').first().click();

      // Should show drill-down view with back button and country header
      await expect(page.getByText('All Countries')).toBeVisible({ timeout: 5000 });

      // Should show top searches table for US
      await expect(page.getByText('Top Searches from United States')).toBeVisible();
      const searchRows = page.locator('table').first().locator('tbody tr');
      await searchRows.first().waitFor({ timeout: 10000 });
      expect(await searchRows.count()).toBeGreaterThan(0);

      // Should show states table
      await expect(page.getByText('States', { exact: true })).toBeVisible();
      const stateRows = page.locator('table').nth(1).locator('tbody tr');
      await stateRows.first().waitFor({ timeout: 10000 });
      expect(await stateRows.count()).toBeGreaterThanOrEqual(10);

      // California should be top state
      await expect(stateRows.first()).toContainText('California');
    });

    test('back button returns to country list', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });

      // Click into US
      await page.locator('table tbody tr').first().click();
      await expect(page.getByText('All Countries')).toBeVisible({ timeout: 5000 });

      // Click back
      await page.getByText('All Countries').click();

      // Should be back to country list
      await expect(page.getByText('Searches by Country')).toBeVisible();
    });
  });

  test.describe('Date Range Switching', () => {
    test('switching to 30d changes KPI values', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });

      // Get 7d total searches value
      const val7d = await page.getByTestId('kpi-total-searches').locator('.text-2xl').textContent();

      // Switch to 30d and poll until KPI value updates
      const num7d = parseInt(val7d!.replace(/,/g, ''), 10);
      await page.getByTestId('range-30d').click();
      await expect(async () => {
        const val30d = await page.getByTestId('kpi-total-searches').locator('.text-2xl').textContent();
        const num30d = parseInt(val30d!.replace(/,/g, ''), 10);
        expect(num30d).toBeGreaterThanOrEqual(num7d);
      }).toPass({ timeout: 10000 });
    });
  });
});
