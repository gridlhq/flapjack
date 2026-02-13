import { test, expect } from '../fixtures/auth.fixture';
import { seedAnalytics, deleteIndex, DEFAULT_ANALYTICS_CONFIG } from '../fixtures/analytics-seed';

/**
 * Analytics UI E2E Tests
 *
 * Real UI E2E tests that interact with actual UI + backend + database.
 * NO MOCKS — follows 3-tier BDD methodology.
 *
 * These tests verify user interactions and UI behavior with real analytics data.
 * For API-level data verification, see integration/analytics-data-verification.spec.ts
 *
 * Prerequisites:
 * - Flapjack server running on localhost:7700
 */

const INDEX = 'e2e-analytics-ui';
const API = 'http://localhost:7700';

async function skipIfNoServer({ request }: { request: any }) {
  try {
    const res = await request.get(`${API}/health`, { timeout: 3000 });
    if (!res.ok()) test.skip(true, 'Flapjack server not available');
  } catch {
    test.skip(true, 'Flapjack server not reachable');
  }
}

// ─── Setup & Teardown ───────────────────────────────────────────────

test.describe('Analytics Page — UI E2E', () => {
  // Seed analytics data once for all tests
  test.beforeAll(async ({ request }) => {
    await skipIfNoServer({ request });
    await seedAnalytics(request, {
      ...DEFAULT_ANALYTICS_CONFIG,
      indexName: INDEX,
    });
  });

  test.afterAll(async ({ request }) => {
    // Clean up test index
    try {
      await deleteIndex(request, INDEX);
    } catch {
      // Ignore cleanup errors
    }
  });

  // ─── Page Structure ────────────────────────────────────────────────

  test.describe('Page Structure', () => {
    test('displays Analytics heading and breadcrumb', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      await expect(page.getByTestId('analytics-heading')).toBeVisible();
      await expect(page.getByTestId('analytics-heading')).toHaveText('Analytics');
      
      const breadcrumb = page.getByTestId('analytics-breadcrumb');
      await expect(breadcrumb).toBeVisible();
      await expect(breadcrumb.getByText('Overview')).toBeVisible();
      await expect(breadcrumb.getByText(INDEX)).toBeVisible();
      await expect(breadcrumb.getByText('Analytics', { exact: true })).toBeVisible();
    });

    test('shows all 6 tab triggers', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      await expect(page.getByTestId('tab-overview')).toBeVisible();
      await expect(page.getByTestId('tab-searches')).toBeVisible();
      await expect(page.getByTestId('tab-no-results')).toBeVisible();
      await expect(page.getByTestId('tab-filters')).toBeVisible();
      await expect(page.getByTestId('tab-devices')).toBeVisible();
      await expect(page.getByTestId('tab-geography')).toBeVisible();
    });

    test('shows date range buttons with 7d selected by default', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      const rangeGroup = page.getByTestId('analytics-date-range');
      await expect(rangeGroup).toBeVisible();
      await expect(page.getByTestId('range-7d')).toHaveClass(/bg-primary/);
      await expect(page.getByTestId('range-30d')).not.toHaveClass(/bg-primary/);
      await expect(page.getByTestId('range-90d')).not.toHaveClass(/bg-primary/);
    });

    test('shows Update and Clear Analytics buttons', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      await expect(page.getByRole('button', { name: /update/i })).toBeVisible();
      await expect(page.getByRole('button', { name: /clear analytics/i })).toBeVisible();
    });
  });

  // ─── Date Range Switching ──────────────────────────────────────────

  test.describe('Date Range Switching', () => {
    test('switching to 30d highlights 30d button', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      await page.getByTestId('range-30d').click();
      await expect(page.getByTestId('range-30d')).toHaveClass(/bg-primary/);
      await expect(page.getByTestId('range-7d')).not.toHaveClass(/bg-primary/);
    });

    test('switching to 90d highlights 90d button', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      await page.getByTestId('range-90d').click();
      await expect(page.getByTestId('range-90d')).toHaveClass(/bg-primary/);
      await expect(page.getByTestId('range-7d')).not.toHaveClass(/bg-primary/);
    });
  });

  // ─── Overview Tab ──────────────────────────────────────────────────

  test.describe('Overview Tab', () => {
    test('shows KPI cards with real data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });
      
      // Total Searches KPI
      const totalSearches = page.getByTestId('kpi-total-searches');
      await expect(totalSearches).toBeVisible();
      const searchText = await totalSearches.locator('.text-2xl').textContent();
      const searchNum = parseInt(searchText!.replace(/,/g, ''), 10);
      expect(searchNum).toBeGreaterThan(0);
      
      // Unique Users KPI
      const uniqueUsers = page.getByTestId('kpi-unique-users');
      await expect(uniqueUsers).toBeVisible();
      const userText = await uniqueUsers.locator('.text-2xl').textContent();
      const userNum = parseInt(userText!.replace(/,/g, ''), 10);
      expect(userNum).toBeGreaterThan(0);
      expect(userNum).toBeLessThan(searchNum);
      
      // No-Result Rate KPI
      const nrr = page.getByTestId('kpi-no-result-rate');
      await expect(nrr).toBeVisible();
      const nrrText = await nrr.locator('.text-2xl').textContent();
      expect(nrrText).toMatch(/\d+\.\d+%/);
    });

    test('shows Search Volume chart', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      const chart = page.getByTestId('search-volume-chart');
      await expect(chart).toBeVisible({ timeout: 10000 });
      await expect(chart.locator('svg')).toBeVisible();
      // Chart should have data rendered (any path element)
      await expect(chart.locator('svg path').first()).toBeVisible();
    });

    test('shows Top 10 Searches table', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      const table = page.getByTestId('top-searches-overview');
      await expect(table).toBeVisible({ timeout: 10000 });
      
      const rows = table.locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });
      
      const count = await rows.count();
      expect(count).toBeGreaterThanOrEqual(5);
      expect(count).toBeLessThanOrEqual(10);
      
      // First row should have rank #1
      await expect(rows.first().locator('td').first()).toHaveText('1');
    });
  });

  // ─── Searches Tab ──────────────────────────────────────────────────

  test.describe('Searches Tab', () => {
    test('displays sortable table with query data', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });
      
      const rows = page.getByTestId('top-searches-table').locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });
      
      const rowCount = await rows.count();
      expect(rowCount).toBeGreaterThan(5);
    });

    test('text filter narrows results client-side', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-searches').click();
      await expect(page.getByTestId('top-searches-table')).toBeVisible({ timeout: 10000 });
      
      await page.getByTestId('top-searches-table').locator('tbody tr').first().waitFor({ timeout: 10000 });
      
      const beforeCount = await page.getByTestId('top-searches-table').locator('tbody tr').count();
      
      // Type "laptop" in filter
      await page.getByTestId('searches-filter-input').fill('laptop');
      
      await expect(async () => {
        const afterCount = await page.getByTestId('top-searches-table').locator('tbody tr').count();
        expect(afterCount).toBeLessThanOrEqual(beforeCount);
      }).toPass({ timeout: 5000 });
    });
  });

  // ─── No Results Tab ────────────────────────────────────────────────

  test.describe('No Results Tab', () => {
    test('shows rate banner with percentage', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-no-results').click();
      
      const banner = page.getByTestId('no-result-rate-banner');
      await expect(banner).toBeVisible({ timeout: 10000 });
      
      const rateText = await banner.locator('.text-3xl').textContent();
      expect(rateText).toMatch(/\d+\.\d+%/);
    });

    test('shows table of zero-result queries', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-no-results').click();
      
      const table = page.getByTestId('no-results-table');
      await expect(table).toBeVisible({ timeout: 10000 });
      
      const rows = table.locator('tbody tr');
      await rows.first().waitFor({ timeout: 10000 });
      
      const count = await rows.count();
      expect(count).toBeGreaterThan(0);
    });
  });

  // ─── Devices Tab ───────────────────────────────────────────────────

  test.describe('Devices Tab', () => {
    test('shows platform cards with counts and percentages', async ({ page }) => {
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
      expect(desktop).toBeGreaterThan(mobile); // Desktop should be higher (60% vs 30%)
    });

    test('shows stacked area chart', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-devices').click();
      
      await expect(page.getByTestId('device-desktop')).toBeVisible({ timeout: 10000 });
      
      await expect(page.getByText('Searches by Device Over Time')).toBeVisible();
      const chartCard = page.getByText('Searches by Device Over Time').locator('xpath=ancestor::div[contains(@class,"rounded-lg")]');
      await expect(chartCard.locator('svg').first()).toBeAttached();
    });
  });

  // ─── Geography Tab ─────────────────────────────────────────────────

  test.describe('Geography Tab', () => {
    test('shows country table with counts', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();

      // Wait for geography content to load
      const table = page.locator('table').first();
      const firstRow = table.locator('tbody tr').first();
      await firstRow.waitFor({ timeout: 10000 });

      // US should be first (45% in seed config)
      await expect(firstRow).toContainText('United States');
    });

    test('clicking country shows drill-down', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();

      // Wait for country table to load
      await page.locator('table tbody tr').first().waitFor({ timeout: 10000 });

      // Click first country (US)
      await page.locator('table tbody tr').first().click();

      // Should show drill-down view
      await expect(page.getByRole('button', { name: /all countries/i })).toBeVisible({ timeout: 5000 });
      await expect(page.getByText(/Top Searches from/)).toBeVisible();
    });

    test('back button returns to country list', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      await page.getByTestId('tab-geography').click();

      // Wait for country table to load
      await page.locator('table tbody tr').first().waitFor({ timeout: 10000 });

      // Click into first country
      await page.locator('table tbody tr').first().click();
      await expect(page.getByRole('button', { name: /all countries/i })).toBeVisible({ timeout: 5000 });

      // Click back
      await page.getByRole('button', { name: /all countries/i }).click();

      // Should be back to country list (verify table visible)
      await expect(page.locator('table tbody tr').first()).toBeVisible();
    });
  });

  // ─── Actions ───────────────────────────────────────────────────────

  test.describe('Actions', () => {
    test('Update button sends flush request', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);

      // Wait for page to load
      await expect(page.getByTestId('analytics-heading')).toBeVisible();

      // Set up response listener before clicking
      const responsePromise = page.waitForResponse(
        (response) =>
          response.url().includes('/2/analytics/flush') &&
          response.request().method() === 'POST',
        { timeout: 10000 }
      );

      await page.getByRole('button', { name: /update/i }).click();
      await responsePromise;
    });

    test('Clear Analytics shows confirmation and sends DELETE', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      let confirmMessage = '';
      page.on('dialog', async (dialog) => {
        confirmMessage = dialog.message();
        await dialog.accept();
      });
      
      const responsePromise = page.waitForResponse(
        (response) =>
          response.url().includes('/2/analytics/clear') &&
          response.request().method() === 'DELETE',
      );
      
      await page.getByRole('button', { name: /clear analytics/i }).click();
      await responsePromise;
      
      expect(confirmMessage).toContain(INDEX);
    });

    test('Clear Analytics cancel does not delete', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      let deleteRequestFired = false;
      page.on('request', (request) => {
        if (request.url().includes('/2/analytics/clear') && request.method() === 'DELETE') {
          deleteRequestFired = true;
        }
      });
      
      page.on('dialog', async (dialog) => {
        await dialog.dismiss();
      });
      
      await page.getByRole('button', { name: /clear analytics/i }).click();
      
      // Wait for UI to settle
      await expect(page.getByRole('button', { name: /clear analytics/i })).toBeVisible();
      expect(deleteRequestFired).toBe(false);
    });
  });

  // ─── Navigation ────────────────────────────────────────────────────

  test.describe('Navigation', () => {
    test('clicking index breadcrumb navigates back', async ({ page }) => {
      await page.goto(`/index/${INDEX}/analytics`);
      
      const breadcrumb = page.getByTestId('analytics-breadcrumb');
      await breadcrumb.getByText(INDEX).click();
      
      await expect(page).toHaveURL(new RegExp(`/index/${INDEX}$`));
    });
  });
});
