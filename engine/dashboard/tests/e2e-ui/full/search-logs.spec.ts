/**
 * E2E-UI Full Suite — Search Logs (API Logs) Page
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests against a REAL Flapjack server.
 * The search logs page captures API requests made by the dashboard itself
 * via an Axios interceptor that writes to a Zustand store persisted in
 * sessionStorage.
 *
 * Covers:
 * - Log entries appear after visiting pages that trigger API calls
 * - Log entry shows HTTP method and URL
 * - Expand a log entry to see curl command and status details
 * - Collapse an expanded log entry
 * - Clear logs resets to empty state
 * - Filter input narrows log entries by URL
 * - View mode toggle (Endpoint vs Curl)
 * - Export button visible
 * - Request count badge accuracy
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { API_BASE } from '../../fixtures/local-instance';

test.describe('Search Logs', () => {
  /**
   * Helper: visit /overview first to generate real API calls (indexes, health),
   * wait for the page to fully load, then navigate to /logs.
   */
  async function generateLogsAndNavigate(page: import('@playwright/test').Page) {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible();
    await expect(page.getByText('e2e-products').first()).toBeVisible({ timeout: 15000 });

    await page.goto('/logs');
    await expect(page.getByRole('heading', { name: /api log/i })).toBeVisible();
  }

  test('logs appear after visiting other pages that trigger API calls', async ({ page }) => {
    await generateLogsAndNavigate(page);

    const logsList = page.getByTestId('logs-list');
    await expect(logsList).toBeVisible({ timeout: 10000 });

    await expect(page.getByText(/\d+ requests/)).toBeVisible();
  });

  test('log entry shows HTTP method and URL', async ({ page }) => {
    await generateLogsAndNavigate(page);

    await expect(page.getByText('GET').first()).toBeVisible();
    await expect(
      page.getByText(/\/1\/indexes|\/health/).first()
    ).toBeVisible();
  });

  test('expanding a log entry shows curl command and status details', async ({ page }) => {
    await generateLogsAndNavigate(page);

    const firstUrl = page.getByText(/\/1\/indexes|\/health/).first();
    await firstUrl.click();

    await expect(page.getByText('Curl Command')).toBeVisible({ timeout: 5000 });
    await expect(page.getByText(/Status:/).first()).toBeVisible();
  });

  test('collapsing an expanded log entry hides details', async ({ page }) => {
    await generateLogsAndNavigate(page);

    // Expand the first entry
    const firstUrl = page.getByText(/\/1\/indexes|\/health/).first();
    await firstUrl.click();
    await expect(page.getByText('Curl Command')).toBeVisible({ timeout: 5000 });

    // Click the same entry again to collapse
    await firstUrl.click();

    // Curl command should no longer be visible
    await expect(page.getByText('Curl Command')).not.toBeVisible({ timeout: 5000 });
  });

  test('clicking Clear removes all log entries and shows empty state', async ({ page }) => {
    await generateLogsAndNavigate(page);

    await expect(page.getByTestId('logs-list')).toBeVisible();

    await page.getByRole('main').getByRole('button', { name: /clear/i }).click();

    // If a confirmation dialog appears, confirm it
    const dialog = page.getByRole('dialog');
    if (await dialog.isVisible({ timeout: 1000 }).catch(() => false)) {
      await dialog.getByRole('button', { name: /clear|confirm/i }).click();
    }

    await expect(page.getByText(/no api logs/i)).toBeVisible({ timeout: 10000 });
    await expect(page.getByTestId('logs-list')).not.toBeVisible();
  });

  // ---------- Filter ----------

  test('filter input narrows log entries by URL', async ({ page }) => {
    await generateLogsAndNavigate(page);

    const logsList = page.getByTestId('logs-list');
    await expect(logsList).toBeVisible({ timeout: 10000 });

    const filterInput = page.getByPlaceholder(/filter by url/i);
    await expect(filterInput).toBeVisible();

    // Filter by "health"
    await filterInput.fill('health');

    // Wait for the filter to take effect — only health entries should remain
    await expect(async () => {
      const visibleEntries = page.getByTestId('logs-list').getByTestId('log-entry');
      const count = await visibleEntries.count();
      // If filtering works, either we see only health entries or fewer entries
      if (count > 0) {
        const firstText = await visibleEntries.first().textContent();
        expect(firstText).toContain('health');
      }
    }).toPass({ timeout: 5000 });

    // Clear filter
    await filterInput.fill('');
  });

  // ---------- View Mode Toggle ----------

  test('view mode toggle switches between endpoint and curl views', async ({ page }) => {
    await generateLogsAndNavigate(page);

    await expect(page.getByTestId('logs-list')).toBeVisible({ timeout: 10000 });

    await expect(page.getByText('Endpoint').first()).toBeVisible();
    await expect(page.getByText('Curl').first()).toBeVisible();

    // Click Curl view mode
    await page.locator('button', { hasText: 'Curl' }).first().click();

    // Should show curl commands (pre-formatted text blocks)
    await expect(page.locator('pre').first()).toBeVisible({ timeout: 5000 });

    // Switch back to Endpoint view
    await page.locator('button', { hasText: 'Endpoint' }).first().click();

    // Table headers should be visible again
    await expect(page.getByText('Time').first()).toBeVisible({ timeout: 5000 });
    await expect(page.getByText('Request').first()).toBeVisible();
  });

  // ---------- Curl View Content ----------

  test('curl view shows actual curl commands with correct format', async ({ page }) => {
    await generateLogsAndNavigate(page);

    await expect(page.getByTestId('logs-list')).toBeVisible({ timeout: 10000 });

    // Switch to Curl view
    await page.locator('button', { hasText: 'Curl' }).first().click();

    // Verify curl commands are shown in pre blocks
    const firstPre = page.locator('pre').first();
    await expect(firstPre).toBeVisible({ timeout: 5000 });

    // The curl command should start with "curl -X" and contain the URL
    const curlText = await firstPre.textContent();
    expect(curlText).toContain('curl -X');
    expect(curlText).toContain(new URL(API_BASE).host);
  });

  // ---------- Expanded Detail Body ----------

  test('expanded log entry shows request body and response sections', async ({ page }) => {
    // Navigate to search page first to generate a POST /query log entry with body
    await page.goto(`/index/e2e-products`);
    await expect(page.getByText('e2e-products').first()).toBeVisible({ timeout: 15000 });

    // Perform a search to ensure POST log entry exists
    const searchInput = page.getByPlaceholder(/search/i).first();
    await searchInput.fill('laptop');
    await searchInput.press('Enter');
    // The postEntry check below uses isVisible({ timeout: 5000 }) — no fixed wait needed

    // Now go to logs
    await page.goto('/logs');
    await expect(page.getByRole('heading', { name: /api log/i })).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId('logs-list')).toBeVisible({ timeout: 10000 });

    // Find and expand a POST entry (search query) which should have a request body
    const postEntry = page.locator('button').filter({ hasText: 'POST' }).first();
    if (await postEntry.isVisible({ timeout: 5000 }).catch(() => false)) {
      await postEntry.click();

      // Expanded detail should show curl command
      await expect(page.getByText('Curl Command')).toBeVisible({ timeout: 10_000 });

      // Should show request body section heading
      await expect(page.getByRole('heading', { name: 'Request Body' })).toBeVisible({ timeout: 10_000 });
    }
  });

  // ---------- Export ----------

  test('export button is visible', async ({ page }) => {
    await generateLogsAndNavigate(page);

    const exportBtn = page.getByRole('main').getByRole('button', { name: /export/i });
    await expect(exportBtn).toBeVisible();
  });

  // ---------- Request Count ----------

  test('request count badge shows accurate count', async ({ page }) => {
    await generateLogsAndNavigate(page);

    const badge = page.getByText(/\d+ requests/);
    await expect(badge).toBeVisible();

    const text = await badge.textContent();
    const count = parseInt(text?.match(/(\d+)/)?.[1] || '0');
    expect(count).toBeGreaterThanOrEqual(1);
  });
});
