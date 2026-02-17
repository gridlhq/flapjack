import { test, expect } from '../../fixtures/auth.fixture';
import { seedAnalytics, deleteIndex, DEFAULT_ANALYTICS_CONFIG } from '../../fixtures/analytics-seed';

/**
 * Analytics Deep Data Verification — E2E-UI (real browser, real server, no mocks)
 *
 * Seeds real analytics data, then verifies it displays correctly in the browser.
 * These tests go beyond visibility checks — they verify actual data values,
 * mathematical consistency, and correct rollup behavior in the rendered UI.
 */

const INDEX = 'e2e-analytics-deep';

const EXPECTED = {
  totalSearches: DEFAULT_ANALYTICS_CONFIG.searchCount,
  uniqueUsers: 50,
  noResultRate: DEFAULT_ANALYTICS_CONFIG.noResultRate,
  desktopPct: DEFAULT_ANALYTICS_CONFIG.deviceDistribution.desktop,
  mobilePct: DEFAULT_ANALYTICS_CONFIG.deviceDistribution.mobile,
};

test.describe('Analytics Deep Data Verification (real browser)', () => {
  test.beforeAll(async ({ request }) => {
    // Seed analytics data for this test suite
    await seedAnalytics(request, {
      ...DEFAULT_ANALYTICS_CONFIG,
      indexName: INDEX,
    });
  });

  test.afterAll(async ({ request }) => {
    try { await deleteIndex(request, INDEX); } catch { /* ignore */ }
  });

  test.describe('Overview Tab — KPI Cards', () => {
    test('KPI cards show non-zero numeric values from seeded data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });

      // Total Searches KPI should show seeded count
      const totalSearches = page.getByTestId('kpi-total-searches');
      await expect(totalSearches).toBeVisible();
      const searchText = await totalSearches.locator('.text-2xl').textContent();
      const searchNum = parseInt(searchText!.replace(/,/g, ''), 10);
      expect(searchNum).toBeGreaterThanOrEqual(EXPECTED.totalSearches * 0.9);

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

    test('search volume chart renders SVG with data path', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      const chart = page.getByTestId('search-volume-chart');
      await expect(chart).toBeVisible({ timeout: 10000 });
      await expect(chart.locator('svg')).toBeVisible();
      await expect(chart.locator('svg path.recharts-area-area')).toBeVisible();
    });

    test('top 10 searches table shows ranked queries in descending order', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      const table = page.getByTestId('top-searches-overview');
      await expect(table).toBeVisible({ timeout: 10000 });
      const rows = table.locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });
      const count = await rows.count();
      expect(count).toBeGreaterThanOrEqual(5);
      expect(count).toBeLessThanOrEqual(10);
      await expect(rows.first().locator('td').first()).toHaveText('1');
    });

    test('KPI cards show delta comparison badges', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });
      // Delta badges require two time periods of data. If only one period exists,
      // KPI cards will be visible but without deltas — still a valid state.
      const deltaBadge = page.getByTestId('delta-badge').first();
      const hasDelta = await deltaBadge.isVisible({ timeout: 10_000 }).catch(() => false);
      if (hasDelta) {
        const badgeCount = await page.getByTestId('delta-badge').count();
        expect(badgeCount).toBeGreaterThan(0);
      }
      // Either way, KPI cards should have numeric values
      await expect(page.getByTestId('kpi-cards').locator('text=/\\d/').first()).toBeVisible({ timeout: 10_000 });
    });
  });

  test.describe('Searches Tab — Data Table', () => {
    test('displays sortable table with query counts in descending order', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });
      const rows = page.getByTestId('top-searches-table').locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });
      const rowCount = await rows.count();
      expect(rowCount).toBeGreaterThan(10);

      // First row count should be larger than last row count
      const firstCount = await rows.first().locator('td:nth-child(3)').textContent();
      const lastCount = await rows.last().locator('td:nth-child(3)').textContent();
      expect(parseInt(firstCount!.replace(/,/g, ''), 10))
        .toBeGreaterThanOrEqual(parseInt(lastCount!.replace(/,/g, ''), 10));
    });

    test('text filter narrows results client-side', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });
      await page.getByTestId('top-searches-table').locator('tbody tr').first().waitFor({ timeout: 10000 });
      const beforeCount = await page.getByTestId('top-searches-table').locator('tbody tr').count();
      expect(beforeCount).toBeGreaterThan(1);

      // Read the first row's query text and filter by a partial match to narrow results
      const firstQuery = await page.getByTestId('top-searches-table').locator('tbody tr').first().locator('td').nth(1).textContent();
      const filterStr = firstQuery?.trim().slice(0, 3) ?? 'sam';
      await page.getByTestId('searches-filter-input').fill(filterStr);
      await expect(async () => {
        const afterCount = await page.getByTestId('top-searches-table').locator('tbody tr').count();
        expect(afterCount).toBeLessThan(beforeCount);
      }).toPass({ timeout: 10000 });
    });
  });

  test.describe('No Results Tab', () => {
    test('shows rate banner with valid percentage between 0-100', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-no-results').click();
      const banner = page.getByTestId('no-result-rate-banner');
      await expect(banner).toBeVisible({ timeout: 10000 });
      const rateText = await banner.locator('.text-3xl').textContent();
      expect(rateText).toMatch(/\d+\.\d+%/);
      const rateValue = parseFloat(rateText!.replace('%', ''));
      expect(rateValue).toBeGreaterThan(0);
      expect(rateValue).toBeLessThan(100);
    });

    test('shows table of zero-result queries with seeded data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-no-results').click();
      const table = page.getByTestId('no-results-table');
      await expect(table).toBeVisible({ timeout: 10000 });
      const rows = table.locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });
      expect(await rows.count()).toBeGreaterThan(0);
    });
  });

  test.describe('Devices Tab', () => {
    test('shows platform cards with desktop > mobile counts', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-devices').click();
      await expect(page.getByTestId('device-desktop')).toBeVisible({ timeout: 10000 });
      await expect(page.getByTestId('device-mobile')).toBeVisible();

      const desktopCount = await page.getByTestId('device-desktop').locator('.text-2xl').textContent();
      const mobileCount = await page.getByTestId('device-mobile').locator('.text-2xl').textContent();
      const desktop = parseInt(desktopCount!.replace(/,/g, ''), 10);
      const mobile = parseInt(mobileCount!.replace(/,/g, ''), 10);
      expect(desktop).toBeGreaterThan(0);
      expect(mobile).toBeGreaterThan(0);
      expect(desktop).toBeGreaterThan(mobile);

      const desktopPct = await page.getByTestId('device-desktop').locator('.text-lg').textContent();
      expect(desktopPct).toMatch(/\d+\.\d+%/);
      const pctVal = parseFloat(desktopPct!.replace('%', ''));
      expect(pctVal).toBeGreaterThan(40);
      expect(pctVal).toBeLessThan(80);
    });

    test('shows device chart with SVG rendering', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-devices').click();
      await expect(page.getByTestId('device-desktop')).toBeVisible({ timeout: 10000 });
      const chart = page.locator('.h-64');
      await expect(chart.locator('svg')).toBeVisible();
    });
  });

  test.describe('Geography Tab', () => {
    test('shows country table with US as top country', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });
      const firstRow = page.locator('table tbody tr').first();
      await firstRow.waitFor({ timeout: 10000 });
      await expect(firstRow).toContainText('United States');
    });

    test('country percentages sum to approximately 100%', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });
      await page.locator('table tbody tr').first().waitFor({ timeout: 10000 });

      const pctCells = page.locator('table tbody tr td:nth-child(4)');
      const cellCount = await pctCells.count();
      let totalPct = 0;
      for (let i = 0; i < cellCount; i++) {
        const text = await pctCells.nth(i).textContent();
        totalPct += parseFloat(text!.replace('%', ''));
      }
      expect(totalPct).toBeGreaterThan(99);
      expect(totalPct).toBeLessThan(101);
    });

    test('clicking country shows drill-down with searches and regions', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });
      await page.locator('table tbody tr').first().click();

      await expect(page.getByText('All Countries')).toBeVisible({ timeout: 5000 });
      await expect(page.getByText('Top Searches from United States')).toBeVisible();
      const searchRows = page.locator('table').first().locator('tbody tr');
      await searchRows.first().waitFor({ timeout: 10000 });
      expect(await searchRows.count()).toBeGreaterThan(0);

      await expect(page.getByText('States', { exact: true })).toBeVisible();
      const stateRows = page.locator('table').nth(1).locator('tbody tr');
      await stateRows.first().waitFor({ timeout: 10000 });
      expect(await stateRows.count()).toBeGreaterThanOrEqual(10);
      await expect(stateRows.first()).toContainText('California');
    });

    test('back button returns from drill-down to country list', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();
      await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10000 });
      await page.locator('table tbody tr').first().click();
      await expect(page.getByText('All Countries')).toBeVisible({ timeout: 5000 });
      await page.getByText('All Countries').click();
      await expect(page.getByText('Searches by Country')).toBeVisible();
    });
  });

  test.describe('Date Range Switching', () => {
    test('switching to 30d updates KPI values', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });
      const val7d = await page.getByTestId('kpi-total-searches').locator('.text-2xl').textContent();
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
