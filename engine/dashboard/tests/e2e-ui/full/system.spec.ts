/**
 * E2E-UI Full Suite — System Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests the System page against a REAL Flapjack server with seeded data.
 * The System page has 4 tabs: Health, Indexes, Replication, Snapshots.
 *
 * Pre-requisites:
 *   - Flapjack server running on port 7700
 *   - `e2e-products` index seeded with 12 products (via seed.setup.ts)
 *   - Vite dev server on localhost:5177
 *
 * Covers:
 *   Health tab:
 *   - Server status shows "ok"
 *   - Active writers count (numeric "N / M" format)
 *   - Facet cache card with numeric values
 *   - Index health summary with green dots
 *   - Auto-refresh notice
 *
 *   Indexes tab:
 *   - e2e-products index visible with document count (12)
 *   - Total indexes, total documents, total storage summary cards
 *   - Index status column shows "Healthy"
 *   - Clicking index link navigates to search page
 *
 *   Replication tab:
 *   - Node ID card visible with value
 *   - Replication status (Enabled/Disabled)
 *   - Auto-refresh notice
 *
 *   Snapshots tab:
 *   - Local Export/Import section with per-index buttons
 *   - Export All button visible
 *   - S3 section (configured or not-configured message)
 *
 *   Tab navigation:
 *   - All four tabs visible and clickable
 *   - Switching tabs updates content
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { TEST_INDEX } from '../helpers';

test.describe('System Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/system');
    await expect(page.getByRole('heading', { name: /system/i })).toBeVisible({ timeout: 10_000 });
  });

  // =========================================================================
  // Health Tab (default tab)
  // =========================================================================

  test('Health tab shows server status as ok', async ({ page }) => {
    const statusCard = page.getByTestId('health-status');
    await expect(statusCard).toBeVisible({ timeout: 15_000 });
    await expect(statusCard.getByText('ok')).toBeVisible();
  });

  test('Health tab shows active writers count', async ({ page }) => {
    const writersCard = page.getByTestId('health-active-writers');
    await expect(writersCard).toBeVisible({ timeout: 15_000 });
    await expect(writersCard.getByText('Active Writers')).toBeVisible();
    // Format: "N / M" where N and M are integers
    await expect(writersCard.getByText(/\d+\s*\/\s*\d+/)).toBeVisible();
  });

  test('Health tab shows facet cache status with numeric values', async ({ page }) => {
    const facetCacheCard = page.getByTestId('health-facet-cache');
    await expect(facetCacheCard).toBeVisible({ timeout: 15_000 });
    await expect(facetCacheCard.getByText('Facet Cache')).toBeVisible();
    // Format: "N / M" entries
    await expect(facetCacheCard.getByText(/\d+\s*\/\s*\d+/)).toBeVisible();
  });

  test('Health tab shows index health summary with green dots', async ({ page }) => {
    const healthSummary = page.getByTestId('index-health-summary');
    await expect(healthSummary).toBeVisible({ timeout: 15_000 });
    await expect(healthSummary.getByText('Index Health')).toBeVisible();

    // e2e-products should appear with a health dot
    const indexDot = page.getByTestId(`index-dot-${TEST_INDEX}`);
    await expect(indexDot).toBeVisible();
    await expect(indexDot.getByText(TEST_INDEX)).toBeVisible();

    // Summary text: "N of M indexes healthy"
    await expect(healthSummary.getByText(/\d+ of \d+ indexes healthy/)).toBeVisible();
  });

  test('Health tab shows auto-refresh notice', async ({ page }) => {
    await expect(page.getByText('Auto-refreshes every 5 seconds')).toBeVisible({ timeout: 10_000 });
  });

  // =========================================================================
  // Indexes Tab
  // =========================================================================

  test('Indexes tab shows e2e-products index with document count', async ({ page }) => {
    await page.getByRole('tab', { name: /indexes/i }).click();

    const indexLink = page.getByTestId('index-link-e2e-products');
    await expect(indexLink).toBeVisible({ timeout: 15_000 });

    // Row should show 12 documents (seeded products)
    await expect(page.getByText('12').first()).toBeVisible();
  });

  test('Indexes tab shows total indexes, documents, and storage cards', async ({ page }) => {
    await page.getByRole('tab', { name: /indexes/i }).click();

    // Total Indexes card
    const totalIndexesCard = page.getByTestId('indexes-total-count');
    await expect(totalIndexesCard).toBeVisible({ timeout: 15_000 });
    await expect(totalIndexesCard.getByText('Total Indexes')).toBeVisible();
    await expect(totalIndexesCard.locator('p.text-2xl')).not.toHaveText('0');

    // Total Documents card
    const totalDocsCard = page.getByTestId('indexes-total-docs');
    await expect(totalDocsCard).toBeVisible();
    await expect(totalDocsCard.getByText('Total Documents')).toBeVisible();
    await expect(totalDocsCard.locator('p.text-2xl')).not.toHaveText('0');

    // Total Storage card
    const totalStorageCard = page.getByTestId('indexes-total-storage');
    await expect(totalStorageCard).toBeVisible();
    await expect(totalStorageCard.getByText('Total Storage')).toBeVisible();
    const storageText = await totalStorageCard.locator('p.text-2xl').textContent();
    expect(storageText).toBeTruthy();
    expect(storageText).not.toBe('0 B');
  });

  test('Indexes tab shows health status column for each index', async ({ page }) => {
    await page.getByRole('tab', { name: /indexes/i }).click();

    const indexStatus = page.getByTestId(`index-status-${TEST_INDEX}`);
    await expect(indexStatus).toBeVisible({ timeout: 15_000 });
    // e2e-products should be healthy (no pending tasks after seeding)
    await expect(indexStatus.getByText('Healthy')).toBeVisible();
  });

  test('clicking index link in Indexes tab navigates to search page', async ({ page }) => {
    await page.getByRole('tab', { name: /indexes/i }).click();
    const indexLink = page.getByTestId('index-link-e2e-products');
    await expect(indexLink).toBeVisible({ timeout: 15_000 });

    await indexLink.click();
    await expect(page).toHaveURL(new RegExp('/index/e2e-products'));
  });

  // =========================================================================
  // Replication Tab
  // =========================================================================

  test('Replication tab shows Node ID card', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();

    // Node ID card should be visible
    const nodeIdHeading = page.getByRole('heading', { name: /node id/i });
    await expect(nodeIdHeading).toBeVisible({ timeout: 15_000 });

    // The Node ID value is displayed in a font-mono paragraph
    await expect(page.locator('p.font-mono').first()).toBeVisible();
  });

  test('Replication tab shows replication enabled/disabled status', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();

    // Should show either "Enabled" or "Disabled" as a large text value
    const enabledText = page.getByText('Enabled', { exact: true });
    const disabledText = page.getByText('Disabled', { exact: true });

    await expect(enabledText.or(disabledText)).toBeVisible({ timeout: 15_000 });
  });

  test('Replication tab shows auto-refresh notice', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();

    await expect(page.getByText('Auto-refreshes every 10 seconds')).toBeVisible({ timeout: 15_000 });
  });

  // =========================================================================
  // Snapshots Tab
  // =========================================================================

  test('Snapshots tab shows Local Export/Import section', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    const snapshotsTab = page.getByTestId('snapshots-tab');
    await expect(snapshotsTab).toBeVisible({ timeout: 15_000 });

    // Local Export/Import heading
    await expect(page.getByText('Local Export / Import')).toBeVisible();

    // Export All button
    const exportAllBtn = page.getByTestId('export-all-btn');
    await expect(exportAllBtn).toBeVisible();
    await expect(exportAllBtn).toContainText('Export All');
  });

  test('Snapshots tab shows per-index export and import buttons', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('snapshots-tab')).toBeVisible({ timeout: 15_000 });

    // Per-index row for e2e-products
    const indexRow = page.getByTestId(`snapshot-index-${TEST_INDEX}`);
    await expect(indexRow).toBeVisible();
    await expect(indexRow.getByText(TEST_INDEX)).toBeVisible();

    // Export and Import buttons for the index
    const exportBtn = page.getByTestId(`export-btn-${TEST_INDEX}`);
    await expect(exportBtn).toBeVisible();
    await expect(exportBtn).toContainText('Export');

    const importBtn = page.getByTestId(`import-btn-${TEST_INDEX}`);
    await expect(importBtn).toBeVisible();
    await expect(importBtn).toContainText('Import');
  });

  test('Snapshots tab shows S3 Backups section', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('snapshots-tab')).toBeVisible({ timeout: 15_000 });

    // S3 section should be visible (may take time to probe S3 availability)
    const s3Section = page.getByTestId('s3-section');
    await expect(s3Section).toBeVisible({ timeout: 15_000 });
    await expect(s3Section.getByRole('heading', { name: /S3 Backups/i })).toBeVisible({ timeout: 5_000 });

    // S3 configured: shows per-index backup/restore buttons
    // S3 not configured: shows config instructions
    const notConfigured = page.getByTestId('s3-not-configured');
    const s3Index = page.getByTestId(`s3-index-${TEST_INDEX}`);
    await expect(notConfigured.or(s3Index)).toBeVisible({ timeout: 10_000 });
  });

  // =========================================================================
  // Tab Navigation
  // =========================================================================

  test('all four tabs are visible and clickable', async ({ page }) => {
    await expect(page.getByRole('tab', { name: /health/i })).toBeVisible();
    await expect(page.getByRole('tab', { name: /indexes/i })).toBeVisible();
    await expect(page.getByRole('tab', { name: /replication/i })).toBeVisible();
    await expect(page.getByRole('tab', { name: /snapshots/i })).toBeVisible();

    // Click Indexes tab and verify content loads
    await page.getByRole('tab', { name: /indexes/i }).click();
    await expect(page.getByTestId('indexes-total-count')).toBeVisible({ timeout: 15_000 });

    // Click back to Health tab and verify content loads
    await page.getByRole('tab', { name: /health/i }).click();
    await expect(page.getByTestId('health-status')).toBeVisible({ timeout: 15_000 });

    // Click Snapshots tab and verify content loads
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('snapshots-tab')).toBeVisible({ timeout: 15_000 });
  });
});
