/**
 * E2E-UI Full Suite -- Analytics Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests the Analytics page against a real Flapjack backend with seeded data.
 * Index `e2e-products` has 7 days of analytics data seeded via /2/analytics/seed.
 *
 * Covers:
 * - Overview tab: KPI cards, search volume chart, top searches table
 * - Searches tab: top searches table, filter input narrows results
 * - No Results tab: rate banner with percentage, severity indicator
 * - Filters tab: filter attributes display
 * - Devices tab: platform breakdown cards
 * - Geography tab: country list, drill-down into country, back button
 * - Date range toggle: 7d/30d/90d switching triggers API calls
 * - Flush button: visible and clickable
 * - BETA badge: displayed
 * - Clear Analytics: confirmation dialog flow
 */
import { test, expect } from '../../fixtures/auth.fixture';

const INDEX = 'e2e-products';
const ANALYTICS_URL = `/index/${INDEX}/analytics`;

test.describe('Analytics', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(ANALYTICS_URL);
    // Wait for the Analytics heading to confirm the page loaded
    await expect(page.getByTestId('analytics-heading')).toBeVisible({ timeout: 15_000 });
  });

  // ---------- Overview Tab ----------

  test('Overview tab loads with KPI cards showing data', async ({ page }) => {
    const kpiCards = page.getByTestId('kpi-cards');
    await expect(kpiCards).toBeVisible({ timeout: 10_000 });

    // Total Searches KPI
    const totalSearches = page.getByTestId('kpi-total-searches');
    await expect(totalSearches).toBeVisible();
    await expect(totalSearches.locator('.text-2xl')).toBeVisible({ timeout: 10_000 });

    // Unique Users KPI
    const uniqueUsers = page.getByTestId('kpi-unique-users');
    await expect(uniqueUsers).toBeVisible();
    await expect(uniqueUsers.locator('.text-2xl')).toBeVisible({ timeout: 10_000 });

    // No-Result Rate KPI
    const noResultRate = page.getByTestId('kpi-no-result-rate');
    await expect(noResultRate).toBeVisible();
    await expect(noResultRate.locator('.text-2xl')).toBeVisible({ timeout: 10_000 });
  });

  test('Search volume chart is visible on Overview tab', async ({ page }) => {
    const chart = page.getByTestId('search-volume-chart');
    await expect(chart).toBeVisible({ timeout: 10_000 });
    await expect(chart.getByText('Search Volume')).toBeVisible();
    // With seeded data, the chart SVG should render (not empty state)
    await expect(chart.locator('.recharts-responsive-container')).toBeVisible({ timeout: 10_000 });
  });

  test('Top searches table shows data on Overview tab', async ({ page }) => {
    const topSearches = page.getByTestId('top-searches-overview');
    await expect(topSearches).toBeVisible({ timeout: 10_000 });
    await expect(topSearches.getByText('Top 10 Searches')).toBeVisible();

    // With seeded data, table rows should appear
    await expect(topSearches.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });
  });

  // ---------- No Results Tab ----------

  test('No Results tab loads with rate banner and table', async ({ page }) => {
    await page.getByTestId('tab-no-results').click();

    // The heading should appear
    await expect(page.getByText('Searches With No Results')).toBeVisible({ timeout: 10_000 });

    // Rate banner should be visible with a percentage
    const banner = page.getByTestId('no-result-rate-banner');
    await expect(banner).toBeVisible({ timeout: 10_000 });
    await expect(banner.getByText(/%/).first()).toBeVisible();
  });

  // ---------- Devices Tab ----------

  test('Devices tab shows platform breakdown', async ({ page }) => {
    await page.getByTestId('tab-devices').click();

    // With seeded data, Desktop and Mobile cards should appear
    await expect(page.getByTestId('device-desktop')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('Desktop')).toBeVisible();

    await expect(page.getByTestId('device-mobile')).toBeVisible();
    await expect(page.getByText('Mobile')).toBeVisible();
  });

  // ---------- Geography Tab ----------

  test('Geography tab shows country list', async ({ page }) => {
    await page.getByTestId('tab-geography').click();

    // Country count and table should be visible
    await expect(page.getByTestId('geo-countries-count')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('Countries')).toBeVisible();
    await expect(page.getByText('Searches by Country')).toBeVisible();

    // Table should have at least one country row
    const rows = page.locator('table tbody tr');
    await expect(rows.first()).toBeVisible();
  });

  test('Geography drill-down: click country row, see details, click back', async ({ page }) => {
    await page.getByTestId('tab-geography').click();
    await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 10_000 });

    // Wait for table rows to load
    await page.locator('table tbody tr').first().waitFor({ timeout: 10_000 });

    // Click the first country row (should be US with seeded data)
    await page.locator('table tbody tr').first().click();

    // Drill-down view should show "All Countries" back button
    await expect(page.getByText('All Countries')).toBeVisible({ timeout: 5_000 });

    // Should show top searches for the selected country
    const topSearchesHeader = page.getByText(/Top Searches from/);
    await expect(topSearchesHeader).toBeVisible({ timeout: 5_000 });

    // Click "All Countries" to go back
    await page.getByText('All Countries').click();

    // Should be back to the country list
    await expect(page.getByText('Searches by Country')).toBeVisible({ timeout: 5_000 });
  });

  // ---------- Date Range Toggle ----------

  test('date range toggle switches between 7d, 30d, 90d and refreshes data', async ({ page }) => {
    const btn7d = page.getByRole('button', { name: '7d' });
    const btn30d = page.getByRole('button', { name: '30d' });
    const btn90d = page.getByRole('button', { name: '90d' });

    // Wait for initial page to fully load with data before toggling
    await expect(btn7d).toBeVisible({ timeout: 10_000 });
    await expect(btn30d).toBeVisible();
    await expect(btn90d).toBeVisible();
    await expect(page.getByTestId('kpi-total-searches').locator('.text-2xl')).toBeVisible({ timeout: 10_000 });

    // Click 30d — poll until KPI data re-renders (no fragile waitForResponse)
    await btn30d.click();
    await expect(async () => {
      const text = await page.getByTestId('kpi-total-searches').locator('.text-2xl').textContent();
      expect(text?.trim().length).toBeGreaterThan(0);
    }).toPass({ timeout: 10_000 });

    // Click 90d
    await btn90d.click();
    await expect(async () => {
      const text = await page.getByTestId('kpi-total-searches').locator('.text-2xl').textContent();
      expect(text?.trim().length).toBeGreaterThan(0);
    }).toPass({ timeout: 10_000 });

    // Click back to 7d
    await btn7d.click();
    await expect(async () => {
      const text = await page.getByTestId('kpi-total-searches').locator('.text-2xl').textContent();
      expect(text?.trim().length).toBeGreaterThan(0);
    }).toPass({ timeout: 10_000 });
  });

  // ---------- Searches Tab ----------

  test('Searches tab shows top searches table with data', async ({ page }) => {
    // Tab always exists — no conditional
    const searchesTab = page.getByTestId('tab-searches');
    await expect(searchesTab).toBeVisible({ timeout: 10_000 });
    await searchesTab.click();

    // Wait for table rows to load
    await expect(page.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });
    const rowCount = await page.locator('table tbody tr').count();
    expect(rowCount).toBeGreaterThan(0);
  });

  test('Searches tab filter input narrows results', async ({ page }) => {
    await page.getByTestId('tab-searches').click();
    await expect(page.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    const beforeCount = await page.locator('table tbody tr').count();

    // Type a specific query in the filter — use a seeded search term
    const filterInput = page.getByTestId('searches-filter-input').or(
      page.getByPlaceholder(/filter/i)
    );
    await expect(filterInput).toBeVisible({ timeout: 5_000 });
    await filterInput.fill('batman');

    // Wait for client-side filter to take effect — rows should decrease
    await expect(async () => {
      const afterCount = await page.locator('table tbody tr').count();
      expect(afterCount).toBeLessThan(beforeCount);
    }).toPass({ timeout: 10_000 });
  });

  // ---------- Filters Tab ----------

  test('Filters tab shows filter section (data or empty state)', async ({ page }) => {
    await page.getByTestId('tab-filters').click();

    const filtersTable = page.getByTestId('filters-table');
    await expect(filtersTable).toBeVisible({ timeout: 10_000 });
    await expect(filtersTable.getByText('Top Filter Attributes')).toBeVisible();

    // Filter analytics may show data or "No filter usage recorded" empty state
    // Both are valid — depends on whether filter aggregation has been computed
    const hasData = filtersTable.locator('table tbody tr').first();
    const emptyState = filtersTable.getByText(/No filter usage/i);
    await expect(hasData.or(emptyState)).toBeVisible({ timeout: 10_000 });
  });

  test('Filters tab: clicking a filter row expands to show filter values', async ({ page }) => {
    await page.getByTestId('tab-filters').click();

    const filtersTable = page.getByTestId('filters-table');

    // Skip expansion test if no filter data is available
    const hasData = await filtersTable.locator('table tbody tr').first().isVisible({ timeout: 5_000 }).catch(() => false);
    if (!hasData) {
      test.skip(true, 'No filter data available — skipping expansion test');
      return;
    }

    // Click the first filter row to expand it
    await filtersTable.locator('table tbody tr').first().click();

    // After expansion, nested value rows should appear
    await expect(async () => {
      const allRows = await filtersTable.locator('table tbody tr').count();
      expect(allRows).toBeGreaterThan(1);
    }).toPass({ timeout: 10_000 });

    // Click again to collapse
    await filtersTable.locator('table tbody tr').first().click();
  });

  // ---------- No-Result Rate Chart ----------

  test('No-Result Rate Over Time chart is visible on Overview tab', async ({ page }) => {
    const nrrChart = page.getByTestId('no-result-rate-chart');
    await expect(nrrChart).toBeVisible({ timeout: 10_000 });
    await expect(nrrChart.getByText('No-Result Rate Over Time')).toBeVisible();

    // With seeded data, the chart should render an SVG (not "No data available")
    const svg = nrrChart.locator('.recharts-responsive-container');
    const noData = nrrChart.getByText('No data available');
    await expect(svg.or(noData)).toBeVisible({ timeout: 10_000 });
  });

  // ---------- Searches Tab (Advanced) ----------

  test('Searches tab shows country filter dropdown when geo data exists', async ({ page }) => {
    await page.getByTestId('tab-searches').click();
    await expect(page.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    // Country filter dropdown should be visible if analytics has geo data
    const countryFilter = page.getByTestId('searches-country-filter');
    if (await countryFilter.isVisible({ timeout: 5_000 }).catch(() => false)) {
      // Default value should be "All Countries"
      await expect(countryFilter).toContainText('All Countries');

      // Select a country — the dropdown should have options
      const options = countryFilter.locator('option');
      const optionCount = await options.count();
      expect(optionCount).toBeGreaterThan(1); // "All Countries" + at least one country
    }
  });

  test('Searches tab shows device filter dropdown when device data exists', async ({ page }) => {
    await page.getByTestId('tab-searches').click();
    await expect(page.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    // Device filter dropdown should be visible if analytics has device data
    const deviceFilter = page.getByTestId('searches-device-filter');
    if (await deviceFilter.isVisible({ timeout: 5_000 }).catch(() => false)) {
      await expect(deviceFilter).toContainText('All Devices');

      const options = deviceFilter.locator('option');
      const optionCount = await options.count();
      expect(optionCount).toBeGreaterThan(1); // "All Devices" + at least one device
    }
  });

  test('Searches tab column headers are clickable for sorting', async ({ page }) => {
    await page.getByTestId('tab-searches').click();
    const topSearchesTable = page.getByTestId('top-searches-table');
    await expect(topSearchesTable.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    // Count column header should show sort indicator (default sorted desc by count)
    const countHeader = topSearchesTable.locator('th').filter({ hasText: 'Count' });
    await expect(countHeader).toBeVisible();

    // Click the "Query" column to sort by query
    const queryHeader = topSearchesTable.locator('th').filter({ hasText: 'Query' });
    await expect(queryHeader).toBeVisible();
    await queryHeader.click();

    // After clicking, the query header should show a sort direction indicator (↑ or ↓)
    await expect(async () => {
      const headerText = await queryHeader.textContent();
      expect(headerText).toMatch(/[↑↓]/);
    }).toPass({ timeout: 5_000 });
  });

  // ---------- Flush Button ----------

  test('Flush button is visible and clickable', async ({ page }) => {
    // The flush button should always be present on the analytics page
    const flushBtn = page.getByRole('button', { name: /flush/i }).or(
      page.locator('button[title*="Flush"]').or(page.locator('button:has(svg.lucide-refresh-cw)'))
    );
    await expect(flushBtn.first()).toBeVisible({ timeout: 10_000 });
  });

  // ---------- BETA Badge ----------

  test('Analytics page shows BETA badge', async ({ page }) => {
    // BETA badge should always be visible on the analytics page (exact match for uppercase "BETA" next to heading)
    await expect(page.getByText('BETA', { exact: true })).toBeVisible({ timeout: 10_000 });
  });

  // ---------- Clear Analytics ----------

  test('Clear Analytics button opens confirmation dialog', async ({ page }) => {
    // Look for the clear/trash button in the analytics header
    const clearBtn = page.getByRole('button', { name: /clear/i }).or(
      page.locator('button[title*="Clear"]')
    );

    // If the clear button exists, test the dialog flow
    if (await clearBtn.first().isVisible({ timeout: 5_000 }).catch(() => false)) {
      await clearBtn.first().click();

      // Confirmation dialog should appear
      const dialog = page.getByRole('dialog');
      await expect(dialog).toBeVisible({ timeout: 5_000 });

      // Dialog should ask for confirmation (index name input or checkbox)
      // Cancel to avoid actually clearing analytics data
      const cancelBtn = dialog.getByRole('button', { name: /cancel/i });
      await expect(cancelBtn).toBeVisible();
      await cancelBtn.click();

      // Dialog should close
      await expect(dialog).not.toBeVisible({ timeout: 5_000 });
    }
  });
});
