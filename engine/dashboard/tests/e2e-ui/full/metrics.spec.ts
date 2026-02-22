/**
 * E2E-UI Full Suite — Metrics Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests the Metrics page against a REAL Flapjack server with seeded data.
 * The Metrics page has 2 tabs: Overview and Per-Index.
 *
 * Pre-requisites:
 *   - Flapjack server running on the repo-local configured backend port
 *   - `e2e-products` index seeded with 12 products (via seed.setup.ts)
 *   - Searches performed against `e2e-products` during seeding
 *   - Vite dev server on the repo-local configured dashboard port
 *
 * Covers:
 *   Overview tab:
 *   - Page heading with Metrics icon
 *   - Version badge and uptime visible
 *   - Aggregate request cards (searches, writes, reads, bytes in) with numeric values
 *   - Aggregate doc/storage cards (total docs, total storage, loaded tenants)
 *   - Auto-refresh notice
 *
 *   Per-Index tab:
 *   - Table visible with at least one index row (e2e-products)
 *   - Doc count > 0 for seeded index
 *   - Storage > 0 for seeded index
 *   - Search count > 0 (seeded searches)
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { TEST_INDEX } from '../helpers';

test.describe('Metrics Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/metrics');
    await expect(page.getByRole('heading', { name: /metrics/i })).toBeVisible({ timeout: 10_000 });
  });

  // =========================================================================
  // Overview Tab (default tab)
  // =========================================================================

  test('Overview tab shows version badge and uptime', async ({ page }) => {
    const versionBadge = page.getByTestId('metrics-version');
    await expect(versionBadge).toBeVisible({ timeout: 15_000 });
    const versionText = await versionBadge.textContent();
    // Should contain a semver-style version like "0.1.0"
    expect(versionText).toMatch(/\d+\.\d+\.\d+/);

    const uptimeEl = page.getByTestId('metrics-uptime');
    await expect(uptimeEl).toBeVisible();
    const uptimeText = await uptimeEl.textContent();
    // Should render "Uptime: Xs", "Uptime: Xm Ys", "Uptime: Xh Ym", or "Uptime: Xd Yh"
    expect(uptimeText).toMatch(/Uptime:\s+\d+[smhd]/);
  });

  test('Overview tab shows aggregate request cards with numeric values', async ({ page }) => {
    // Total Searches card
    const searchesCard = page.getByTestId('metrics-total-searches');
    await expect(searchesCard).toBeVisible({ timeout: 15_000 });
    await expect(searchesCard.getByText('Total Searches')).toBeVisible();
    const searchValue = await searchesCard.getByTestId('stat-value').textContent();
    // seed.setup.ts performs at least one search — this must be > 0
    expect(Number(searchValue?.replace(/,/g, ''))).toBeGreaterThan(0);

    // Total Writes card
    const writesCard = page.getByTestId('metrics-total-writes');
    await expect(writesCard).toBeVisible();
    await expect(writesCard.getByText('Total Writes')).toBeVisible();
    const writeValue = await writesCard.getByTestId('stat-value').textContent();
    expect(Number(writeValue?.replace(/,/g, ''))).toBeGreaterThanOrEqual(0);

    // Total Reads card
    const readsCard = page.getByTestId('metrics-total-reads');
    await expect(readsCard).toBeVisible();
    await expect(readsCard.getByText('Total Reads')).toBeVisible();

    // Total Bytes In card
    const bytesCard = page.getByTestId('metrics-total-bytes-in');
    await expect(bytesCard).toBeVisible();
    await expect(bytesCard.getByText('Total Bytes In')).toBeVisible();
  });

  test('Overview tab shows aggregate doc/storage cards', async ({ page }) => {
    // Total Documents card
    const docsCard = page.getByTestId('metrics-total-docs');
    await expect(docsCard).toBeVisible({ timeout: 15_000 });
    await expect(docsCard.getByText('Total Documents')).toBeVisible();
    const docValue = await docsCard.getByTestId('stat-value').textContent();
    expect(Number(docValue?.replace(/,/g, ''))).toBeGreaterThanOrEqual(12);

    // Total Storage card
    const storageCard = page.getByTestId('metrics-total-storage');
    await expect(storageCard).toBeVisible();
    await expect(storageCard.getByText('Total Storage')).toBeVisible();
    const storageValue = await storageCard.getByTestId('stat-value').textContent();
    expect(storageValue).toBeTruthy();
    expect(storageValue).not.toBe('0 Bytes');

    // Loaded Tenants card
    const tenantsCard = page.getByTestId('metrics-tenants');
    await expect(tenantsCard).toBeVisible();
    await expect(tenantsCard.getByText('Loaded Tenants')).toBeVisible();
    const tenantValue = await tenantsCard.getByTestId('stat-value').textContent();
    expect(Number(tenantValue?.replace(/,/g, ''))).toBeGreaterThanOrEqual(1);
  });

  test('Overview tab shows auto-refresh notice', async ({ page }) => {
    await expect(page.getByText('Auto-refreshes every 10 seconds')).toBeVisible({ timeout: 10_000 });
  });

  // =========================================================================
  // Per-Index Tab
  // =========================================================================

  test('Per-Index tab shows table with seeded index row', async ({ page }) => {
    await page.getByRole('tab', { name: /per-index/i }).click();

    const table = page.getByTestId('metrics-per-index-table');
    await expect(table).toBeVisible({ timeout: 15_000 });

    // Verify the seeded e2e-products index appears as a row
    const indexRow = page.getByTestId(`metrics-index-row-${TEST_INDEX}`);
    await expect(indexRow).toBeVisible();
    await expect(indexRow.getByText(TEST_INDEX)).toBeVisible();
  });

  test('Per-Index tab shows doc count >= 12 and storage > 0 for seeded index', async ({ page }) => {
    await page.getByRole('tab', { name: /per-index/i }).click();

    await expect(page.getByTestId(`metrics-index-row-${TEST_INDEX}`)).toBeVisible({ timeout: 15_000 });

    // Documents cell — should be >= 12 (seeded products)
    const docCell = page.getByTestId(`metrics-cell-${TEST_INDEX}-docs`);
    await expect(docCell).toBeVisible();
    const docText = await docCell.textContent();
    expect(Number(docText?.replace(/,/g, ''))).toBeGreaterThanOrEqual(12);

    // Storage cell — should not be "0 Bytes"
    const storageCell = page.getByTestId(`metrics-cell-${TEST_INDEX}-storage`);
    await expect(storageCell).toBeVisible();
    const storageText = await storageCell.textContent();
    expect(storageText).toBeTruthy();
    expect(storageText).not.toBe('0 Bytes');
  });

  test('Per-Index tab shows search count > 0 for seeded index', async ({ page }) => {
    await page.getByRole('tab', { name: /per-index/i }).click();

    await expect(page.getByTestId(`metrics-index-row-${TEST_INDEX}`)).toBeVisible({ timeout: 15_000 });

    // Search count cell — seed.setup.ts performs searches against e2e-products
    const searchCell = page.getByTestId(`metrics-cell-${TEST_INDEX}-searches`);
    await expect(searchCell).toBeVisible();
    const searchText = await searchCell.textContent();
    expect(Number(searchText?.replace(/,/g, ''))).toBeGreaterThan(0);
  });

  test('Per-Index tab shows auto-refresh notice', async ({ page }) => {
    await page.getByRole('tab', { name: /per-index/i }).click();
    await expect(page.getByText('Auto-refreshes every 10 seconds')).toBeVisible({ timeout: 10_000 });
  });

  test('Per-Index tab column sort: clicking column header updates aria-sort state', async ({ page }) => {
    await page.getByRole('tab', { name: /per-index/i }).click();
    await expect(page.getByTestId('metrics-per-index-table')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId(`metrics-index-row-${TEST_INDEX}`)).toBeVisible();

    const docsHeader = page.getByRole('columnheader', { name: /documents/i });
    const nameHeader = page.getByRole('columnheader', { name: /index name/i });

    // Initial state: Index Name sorted ascending, Documents unsorted
    await expect(nameHeader).toHaveAttribute('aria-sort', 'ascending');
    await expect(docsHeader).toHaveAttribute('aria-sort', 'none');

    // First click on Documents → ascending
    await docsHeader.click();
    await expect(docsHeader).toHaveAttribute('aria-sort', 'ascending');
    await expect(nameHeader).toHaveAttribute('aria-sort', 'none');

    // Second click on Documents → descending
    await docsHeader.click();
    await expect(docsHeader).toHaveAttribute('aria-sort', 'descending');

    // Click Index Name to restore default sort
    await nameHeader.click();
    await expect(nameHeader).toHaveAttribute('aria-sort', 'ascending');
    await expect(docsHeader).toHaveAttribute('aria-sort', 'none');

    // Seeded index must remain visible throughout
    await expect(page.getByTestId(`metrics-index-row-${TEST_INDEX}`)).toBeVisible();
  });

  // =========================================================================
  // Tab Navigation
  // =========================================================================

  test('both tabs are visible and clickable', async ({ page }) => {
    await expect(page.getByRole('tab', { name: /overview/i })).toBeVisible();
    await expect(page.getByRole('tab', { name: /per-index/i })).toBeVisible();

    // Click Per-Index tab and verify table loads
    await page.getByRole('tab', { name: /per-index/i }).click();
    await expect(page.getByTestId('metrics-per-index-table')).toBeVisible({ timeout: 15_000 });

    // Click back to Overview tab and verify cards load
    await page.getByRole('tab', { name: /overview/i }).click();
    await expect(page.getByTestId('metrics-total-searches')).toBeVisible({ timeout: 15_000 });
  });
});
