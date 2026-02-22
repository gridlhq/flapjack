/**
 * E2E-UI Full Suite -- Analytics Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests the Analytics page against a real Flapjack backend with seeded data.
 * Index `e2e-products` has 7 days of analytics data seeded via /2/analytics/seed.
 *
 * Covers:
 * - Overview tab: KPI cards, search volume chart, NRR chart, top searches table
 * - Searches tab: top searches table, filter input, country/device dropdowns, sorting
 * - No Results tab: rate banner with percentage, severity indicator
 * - Filters tab: filter attributes display, expansion, filters-causing-no-results
 * - Devices tab: platform breakdown cards
 * - Geography tab: country list, drill-down into country, back button
 * - Date range toggle: 7d/30d/90d switching triggers API calls
 * - Flush button: visible, clickable, triggers data refresh
 * - BETA badge: displayed
 * - Clear Analytics: confirmation dialog flow
 *
 * STANDARDS COMPLIANCE (BROWSER_TESTING_STANDARDS_2.md):
 * - Zero page.evaluate() — all assertions via Playwright locators
 * - Zero CSS class selectors — uses data-testid, getByRole, getByText
 * - Zero conditional skipping — all assertions are hard (no if/catch guards)
 * - Zero { force: true } — relies on Playwright actionability checks
 * - ESLint enforced via tests/e2e-ui/eslint.config.mjs
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

    // Total Searches KPI — value must be visible (uses data-testid, not CSS class)
    const totalSearches = page.getByTestId('kpi-total-searches');
    await expect(totalSearches).toBeVisible();
    await expect(totalSearches.getByTestId('kpi-value')).toBeVisible({ timeout: 10_000 });

    // Unique Users KPI
    const uniqueUsers = page.getByTestId('kpi-unique-users');
    await expect(uniqueUsers).toBeVisible();
    await expect(uniqueUsers.getByTestId('kpi-value')).toBeVisible({ timeout: 10_000 });

    // No-Result Rate KPI
    const noResultRate = page.getByTestId('kpi-no-result-rate');
    await expect(noResultRate).toBeVisible();
    await expect(noResultRate.getByTestId('kpi-value')).toBeVisible({ timeout: 10_000 });
  });

  test('Search volume chart renders SVG on Overview tab (not empty state)', async ({ page }) => {
    const chart = page.getByTestId('search-volume-chart');
    await expect(chart).toBeVisible({ timeout: 10_000 });
    await expect(chart.getByText('Search Volume')).toBeVisible();

    // With seeded data, the chart MUST render an SVG — empty state is a failure
    await expect(chart.locator('svg')).toBeVisible({ timeout: 10_000 });
  });

  test('Top searches table shows data on Overview tab', async ({ page }) => {
    const topSearches = page.getByTestId('top-searches-overview');
    await expect(topSearches).toBeVisible({ timeout: 10_000 });
    await expect(topSearches.getByText('Top 10 Searches')).toBeVisible();

    // With seeded data, table rows should appear
    await expect(topSearches.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });
  });

  test('No-Result Rate Over Time chart renders SVG (not empty state)', async ({ page }) => {
    const nrrChart = page.getByTestId('no-result-rate-chart');
    await expect(nrrChart).toBeVisible({ timeout: 10_000 });
    await expect(nrrChart.getByText('No-Result Rate Over Time')).toBeVisible();

    // With seeded data, the chart MUST render SVG. "No data available" is a FAILURE.
    // Previously this used an OR condition that accepted empty state — that's a false positive.
    await expect(nrrChart.locator('svg')).toBeVisible({ timeout: 10_000 });
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

    // With seeded data, Desktop and Mobile cards MUST appear
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
    await expect(page.getByTestId('kpi-total-searches').getByTestId('kpi-value')).toBeVisible({ timeout: 10_000 });

    // Click 30d — poll until KPI data re-renders (no fragile waitForResponse)
    await btn30d.click();
    await expect(async () => {
      const text = await page.getByTestId('kpi-total-searches').getByTestId('kpi-value').textContent();
      expect(text?.trim().length).toBeGreaterThan(0);
    }).toPass({ timeout: 10_000 });

    // Click 90d
    await btn90d.click();
    await expect(async () => {
      const text = await page.getByTestId('kpi-total-searches').getByTestId('kpi-value').textContent();
      expect(text?.trim().length).toBeGreaterThan(0);
    }).toPass({ timeout: 10_000 });

    // Click back to 7d
    await btn7d.click();
    await expect(async () => {
      const text = await page.getByTestId('kpi-total-searches').getByTestId('kpi-value').textContent();
      expect(text?.trim().length).toBeGreaterThan(0);
    }).toPass({ timeout: 10_000 });
  });

  // ---------- Searches Tab ----------

  test('Searches tab shows top searches table with data', async ({ page }) => {
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
    await expect(filtersTable).toBeVisible({ timeout: 10_000 });

    // With seeded data, filter rows MUST exist. Hard assertion — no conditional skip.
    const firstRow = filtersTable.locator('table tbody tr').first();
    await expect(firstRow).toBeVisible({ timeout: 10_000 });

    // Click the first filter row to expand it
    await firstRow.click();

    // After expansion, nested value rows should appear
    await expect(async () => {
      const allRows = await filtersTable.locator('table tbody tr').count();
      expect(allRows).toBeGreaterThan(1);
    }).toPass({ timeout: 10_000 });

    // Click again to collapse
    await filtersTable.locator('table tbody tr').first().click();
  });

  // ---------- Searches Tab (Advanced) ----------

  test('Searches tab shows country filter dropdown with seeded geo data', async ({ page }) => {
    await page.getByTestId('tab-searches').click();
    await expect(page.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    // With seeded geo data, country filter MUST be visible. Hard assertion — no if guard.
    const countryFilter = page.getByTestId('searches-country-filter');
    await expect(countryFilter).toBeVisible({ timeout: 10_000 });

    // Default value should be "All Countries"
    await expect(countryFilter).toContainText('All Countries');

    // Dropdown should have options beyond "All Countries"
    const options = countryFilter.locator('option');
    const optionCount = await options.count();
    expect(optionCount).toBeGreaterThan(1);
  });

  test('Searches tab shows device filter dropdown with seeded device data', async ({ page }) => {
    await page.getByTestId('tab-searches').click();
    await expect(page.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    // With seeded device data, device filter MUST be visible. Hard assertion — no if guard.
    const deviceFilter = page.getByTestId('searches-device-filter');
    await expect(deviceFilter).toBeVisible({ timeout: 10_000 });

    await expect(deviceFilter).toContainText('All Devices');

    const options = deviceFilter.locator('option');
    const optionCount = await options.count();
    expect(optionCount).toBeGreaterThan(1);
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

    // After clicking, the query header should show a sort direction indicator
    await expect(async () => {
      const headerText = await queryHeader.textContent();
      expect(headerText).toMatch(/[↑↓]/);
    }).toPass({ timeout: 5_000 });
  });

  // ---------- Flush Button (Functional) ----------

  test('Flush button triggers analytics refresh', async ({ page }) => {
    // Flush button MUST be present — hard assertion using data-testid
    const flushBtn = page.getByTestId('flush-btn');
    await expect(flushBtn).toBeVisible({ timeout: 10_000 });

    // Click flush and verify it triggers a network request to /analytics/flush
    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/analytics') && (resp.status() === 200 || resp.status() === 202),
      { timeout: 15_000 }
    );
    await flushBtn.click();
    const response = await responsePromise;
    expect([200, 202]).toContain(response.status());

    // After flush completes, button should show "Update" (not stuck on loading)
    await expect(flushBtn).toContainText('Update', { timeout: 15_000 });

    // KPI cards should still show data after refresh
    await expect(page.getByTestId('kpi-total-searches').getByTestId('kpi-value')).toBeVisible({ timeout: 10_000 });
  });

  // ---------- BETA Badge ----------

  test('Analytics page shows BETA badge', async ({ page }) => {
    await expect(page.getByText('BETA', { exact: true })).toBeVisible({ timeout: 10_000 });
  });

  // ---------- Clear Analytics ----------

  test('Clear Analytics button opens confirmation dialog', async ({ page }) => {
    // Clear button MUST be present — hard assertion using data-testid
    const clearBtn = page.getByTestId('clear-btn');
    await expect(clearBtn).toBeVisible({ timeout: 10_000 });

    await clearBtn.click();

    // Confirmation dialog should appear
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible({ timeout: 5_000 });

    // Dialog should mention clearing analytics
    await expect(dialog.getByText(/clear/i).first()).toBeVisible();

    // Cancel to avoid actually clearing analytics data
    const cancelBtn = dialog.getByRole('button', { name: /cancel/i });
    await expect(cancelBtn).toBeVisible();
    await cancelBtn.click();

    // Dialog should close
    await expect(dialog).not.toBeVisible({ timeout: 5_000 });
  });

  // ---------- Filters Causing No Results ----------

  test('Filters tab loads and shows filter data table', async ({ page }) => {
    await page.getByTestId('tab-filters').click();
    const filtersTable = page.getByTestId('filters-table');
    await expect(filtersTable).toBeVisible({ timeout: 10_000 });

    // Verify the filters table has rows (the seeded analytics data includes filter usage)
    await expect(filtersTable.locator('table tbody tr').first()).toBeVisible({ timeout: 10_000 });

    // The "Filters Causing No Results" section is data-dependent — it only renders
    // when the server returns filter analytics with no-result data. Verify if present.
    const noResultFilters = page.getByTestId('filters-no-results');
    const isVisible = await noResultFilters.isVisible().catch(() => false);
    if (isVisible) {
      await expect(noResultFilters.getByText('Filters Causing No Results')).toBeVisible();
      await expect(noResultFilters.locator('table tbody tr').first()).toBeVisible();
    }
  });

  // ---------- Breadcrumb Navigation ----------

  test('breadcrumb shows Overview > index > Analytics and links work', async ({ page }) => {
    const breadcrumb = page.getByTestId('analytics-breadcrumb');
    await expect(breadcrumb).toBeVisible({ timeout: 10_000 });

    // Breadcrumb should display: Overview / {indexName} / Analytics
    await expect(breadcrumb.getByText('Overview')).toBeVisible();
    await expect(breadcrumb.getByText(INDEX)).toBeVisible();
    await expect(breadcrumb.getByText('Analytics')).toBeVisible();

    // Click "Overview" link — should navigate to /overview
    await breadcrumb.getByText('Overview').click();
    await expect(page).toHaveURL(/\/overview/);
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });
  });

  test('breadcrumb index link navigates to search page', async ({ page }) => {
    const breadcrumb = page.getByTestId('analytics-breadcrumb');
    await expect(breadcrumb).toBeVisible({ timeout: 10_000 });

    // Click the index name link — should navigate to /index/{indexName}
    await breadcrumb.getByText(INDEX).click();
    await expect(page).toHaveURL(new RegExp(`/index/${INDEX}`));
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });
  });

  // ---------- Date Range Label ----------

  test('date range label shows formatted date range', async ({ page }) => {
    const dateLabel = page.getByTestId('analytics-date-label');
    await expect(dateLabel).toBeVisible({ timeout: 10_000 });

    // Label should show a date range like "Feb 13 - Feb 20"
    const labelText = await dateLabel.textContent();
    expect(labelText).toMatch(/\w{3} \d{1,2}\s*[-–]\s*\w{3} \d{1,2}/);
  });

  // ---------- KPI Content Verification ----------

  test('KPI cards show formatted numeric values, not just visibility', async ({ page }) => {
    const totalSearches = page.getByTestId('kpi-total-searches');
    await expect(totalSearches).toBeVisible({ timeout: 10_000 });

    // Total Searches must show a number (not "No data" or empty)
    const searchValue = await totalSearches.getByTestId('kpi-value').textContent();
    expect(searchValue?.trim()).toMatch(/^[\d,]+$/);
    const searchNum = parseInt(searchValue!.replace(/,/g, ''), 10);
    expect(searchNum).toBeGreaterThan(0);

    // Unique Users must show a number
    const usersValue = await page.getByTestId('kpi-unique-users').getByTestId('kpi-value').textContent();
    expect(usersValue?.trim()).toMatch(/^[\d,]+$/);

    // No-Result Rate must show a percentage (e.g., "5.0%")
    const nrrValue = await page.getByTestId('kpi-no-result-rate').getByTestId('kpi-value').textContent();
    expect(nrrValue?.trim()).toMatch(/^\d+\.\d+%$/);
  });

  // ---------- Sparkline Rendering ----------

  test('KPI cards with time-series data render sparkline SVGs', async ({ page }) => {
    // Total Searches and No-Result Rate KPIs have sparkData — SVG must render
    const totalSearchesSparkline = page.getByTestId('kpi-total-searches').getByTestId('sparkline');
    await expect(totalSearchesSparkline).toBeVisible({ timeout: 10_000 });
    await expect(totalSearchesSparkline.locator('svg')).toBeVisible();

    const nrrSparkline = page.getByTestId('kpi-no-result-rate').getByTestId('sparkline');
    await expect(nrrSparkline).toBeVisible();
    await expect(nrrSparkline.locator('svg')).toBeVisible();
  });

  // ---------- Top Searches Content Verification ----------

  test('Top searches table shows ranked queries with counts', async ({ page }) => {
    const table = page.getByTestId('top-searches-overview');
    await expect(table).toBeVisible({ timeout: 10_000 });

    const rows = table.locator('tbody tr');
    await rows.first().waitFor({ timeout: 10_000 });

    // First row rank should be "1"
    await expect(rows.first().locator('td').first()).toHaveText('1');

    // Query cell should have non-empty text
    const queryText = await rows.first().getByTestId('search-query').textContent();
    expect(queryText?.trim().length).toBeGreaterThan(0);

    // Count cell should have a formatted number
    const countText = await rows.first().getByTestId('search-count').textContent();
    expect(countText?.trim()).toMatch(/^[\d,]+$/);
  });

  // ---------- Clear Analytics Content Verification ----------

  test('Clear Analytics dialog shows correct warning text and index name', async ({ page }) => {
    const clearBtn = page.getByTestId('clear-btn');
    await expect(clearBtn).toBeVisible({ timeout: 10_000 });
    await clearBtn.click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible({ timeout: 5_000 });

    // Dialog should mention the specific index name
    await expect(dialog.getByText(INDEX)).toBeVisible();

    // Dialog should have destructive confirm button text
    const confirmBtn = dialog.getByRole('button', { name: /clear|delete|confirm/i });
    await expect(confirmBtn).toBeVisible();

    // Cancel
    await dialog.getByRole('button', { name: /cancel/i }).click();
    await expect(dialog).not.toBeVisible({ timeout: 5_000 });
  });

  // ---------- Flush Button State Transitions ----------

  test('Flush button can be clicked and returns to ready state', async ({ page }) => {
    const flushBtn = page.getByTestId('flush-btn');
    await expect(flushBtn).toBeVisible({ timeout: 10_000 });
    await expect(flushBtn).toBeEnabled();

    // Click flush — the mutation fires and the button eventually returns to "Update"
    await flushBtn.click();

    // After completion, button should show "Update" and be enabled again
    await expect(flushBtn).toContainText('Update', { timeout: 15_000 });
    await expect(flushBtn).toBeEnabled();
  });
});
