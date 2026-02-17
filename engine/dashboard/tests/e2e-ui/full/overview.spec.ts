/**
 * E2E-UI Full Suite — Overview Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests run against a REAL Flapjack server with seeded test data.
 * The 'e2e-products' index is pre-seeded with 12 products.
 *
 * Covers:
 * - Index list with seeded index and document count
 * - Stat cards (indexes, documents, storage, health)
 * - Health indicator
 * - Create/delete index lifecycle
 * - Create Index dialog templates
 * - Export All and Upload buttons
 * - Per-index export/import buttons
 * - Index row storage and timestamp info
 * - Analytics summary section
 * - Settings link navigation
 * - Clicking index navigates to search page
 * - Export All triggers download
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { API_BASE, API_HEADERS, TEST_INDEX } from '../helpers';

test.describe('Overview Page', () => {

  test.beforeEach(async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10000 });
  });

  test('index list shows e2e-products with document count', async ({ page }) => {
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible({ timeout: 10000 });
    await expect(page.getByText(/12 documents/)).toBeVisible();
  });

  test('stat cards display total indexes, documents, and storage', async ({ page }) => {
    const indexesCard = page.getByTestId('stat-card-indexes');
    await expect(indexesCard).toBeVisible();
    const indexCount = await indexesCard.locator('.text-2xl').textContent();
    expect(Number(indexCount)).toBeGreaterThanOrEqual(1);

    const docsCard = page.getByTestId('stat-card-documents');
    await expect(docsCard).toBeVisible();
    const docCount = await docsCard.locator('.text-2xl').textContent();
    expect(Number(docCount?.replace(/,/g, ''))).toBeGreaterThanOrEqual(12);

    const storageCard = page.getByTestId('stat-card-storage');
    await expect(storageCard).toBeVisible();
    const storageText = await storageCard.locator('.text-2xl').textContent();
    expect(storageText).toBeTruthy();
    expect(storageText).not.toBe('0 B');
  });

  test('health indicator shows Healthy', async ({ page }) => {
    const statusCard = page.getByTestId('stat-card-status');
    await expect(statusCard).toBeVisible();
    await expect(statusCard.getByText('Healthy')).toBeVisible({ timeout: 10000 });
  });

  test('create new index e2e-temp, verify it appears, then delete it', async ({ page, request }) => {
    const tempIndex = 'e2e-temp';

    await request.delete(`${API_BASE}/1/indexes/${tempIndex}`, { headers: API_HEADERS }).catch(() => {});

    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('heading', { name: 'Create Index' })).toBeVisible();

    await dialog.locator('#index-uid').fill(tempIndex);
    await dialog.getByRole('button', { name: /create index/i }).click();

    await expect(dialog).not.toBeVisible({ timeout: 10000 });
    await expect(page.getByText(tempIndex).first()).toBeVisible({ timeout: 10000 });

    const deleteBtn = page.getByTitle(`Delete index "${tempIndex}"`);
    while (await deleteBtn.count() === 0) {
      const nextBtn = page.getByRole('button', { name: /next/i });
      if (await nextBtn.isEnabled()) {
        await nextBtn.click();
        await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 5000 });
      } else {
        break;
      }
    }
    await deleteBtn.click();

    const confirmDialog = page.getByRole('dialog');
    await expect(confirmDialog).toBeVisible();
    await expect(confirmDialog.getByText(/are you sure/i)).toBeVisible();
    await confirmDialog.getByRole('button', { name: /delete/i }).click();

    await expect(confirmDialog).not.toBeVisible({ timeout: 10000 });

    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible();
    await expect(page.getByText(tempIndex)).not.toBeVisible({ timeout: 5000 });
  });

  test('server health badge shows connected status', async ({ page }) => {
    const statusCard = page.getByTestId('stat-card-status');
    await expect(statusCard).toBeVisible();
    await expect(statusCard.getByText('Healthy')).toBeVisible({ timeout: 10000 });
    await expect(statusCard.getByText('Disconnected')).not.toBeVisible();
  });

  test('clicking e2e-products navigates to its search page', async ({ page }) => {
    await page.getByText(TEST_INDEX).first().click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}`));
  });

  test('Create Index dialog shows template options', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await expect(dialog.getByText('Empty index')).toBeVisible();
    await expect(dialog.getByText(/Movies/)).toBeVisible();
    await expect(dialog.getByText(/Products/)).toBeVisible();
    await expect(dialog.locator('#index-uid')).toBeVisible();

    await dialog.getByRole('button', { name: /cancel/i }).click();
    await expect(dialog).not.toBeVisible({ timeout: 5000 });
  });

  test('selecting Movies template auto-fills index name', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByText(/Movies/).click();
    const nameInput = dialog.locator('#index-uid');
    await expect(nameInput).toHaveValue('movies');

    await dialog.getByRole('button', { name: /cancel/i }).click();
  });

  test('Export All and Upload buttons are visible', async ({ page }) => {
    const exportBtn = page.getByTestId('overview-export-all-btn');
    await expect(exportBtn).toBeVisible();
    await expect(exportBtn).toContainText('Export All');

    const uploadBtn = page.getByTestId('overview-upload-btn');
    await expect(uploadBtn).toBeVisible();
    await expect(uploadBtn).toContainText('Upload');
  });

  test('per-index export and import buttons are visible', async ({ page }) => {
    const exportBtn = page.getByTestId(`overview-export-${TEST_INDEX}`);
    await expect(exportBtn).toBeVisible();

    const importBtn = page.getByTestId(`overview-import-${TEST_INDEX}`);
    await expect(importBtn).toBeVisible();
  });

  test('index row shows storage size and update info', async ({ page }) => {
    await expect(page.getByText(/12 documents/).first()).toBeVisible();
    await expect(page.getByText(/\d+(\.\d+)?\s*(B|KB|MB|GB)/i).first()).toBeVisible();
  });

  test('search analytics section displays data from seeded analytics', async ({ page }) => {
    const analyticsCard = page.getByTestId('overview-analytics');
    await expect(analyticsCard).toBeVisible({ timeout: 10_000 });
    await expect(analyticsCard.getByText('Search Analytics')).toBeVisible();
    await expect(analyticsCard.getByText('Total Searches')).toBeVisible();
    await expect(analyticsCard.getByText('Unique Users')).toBeVisible();
    await expect(analyticsCard.getByText('No-Result Rate')).toBeVisible();
  });

  test('analytics chart renders in the overview analytics section', async ({ page }) => {
    const analyticsCard = page.getByTestId('overview-analytics');
    await expect(analyticsCard).toBeVisible({ timeout: 10_000 });

    // With seeded data, the mini chart SVG should render
    const chart = analyticsCard.locator('.recharts-responsive-container');
    await expect(chart).toBeVisible({ timeout: 10_000 });
  });

  test('View Details link in analytics section navigates to analytics page', async ({ page }) => {
    const analyticsCard = page.getByTestId('overview-analytics');
    await expect(analyticsCard).toBeVisible({ timeout: 10_000 });

    const viewDetailsLink = analyticsCard.getByText('View Details');
    await expect(viewDetailsLink).toBeVisible();
    await viewDetailsLink.click();

    await expect(page).toHaveURL(/\/analytics/);
    await expect(page.getByTestId('analytics-heading')).toBeVisible({ timeout: 10_000 });
  });

  test('Settings button in index row navigates to settings', async ({ page }) => {
    // Find the specific Settings link for our test index (not other indexes on the page)
    const settingsLink = page.getByRole('link', { name: /settings/i }).and(
      page.locator(`[href*="${TEST_INDEX}/settings"]`)
    );
    await expect(settingsLink).toBeVisible({ timeout: 10_000 });
    await settingsLink.click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}/settings`));
  });

  test('Export All button triggers download', async ({ page }) => {
    const exportBtn = page.getByTestId('overview-export-all-btn');
    await expect(exportBtn).toBeVisible();

    const downloadPromise = page.waitForEvent('download', { timeout: 15_000 }).catch(() => null);
    const responsePromise = page.waitForResponse(
      resp => resp.url().includes('/snapshot') || resp.url().includes('/export'),
      { timeout: 15_000 }
    ).catch(() => null);

    await exportBtn.click();

    const download = await downloadPromise;
    const response = await responsePromise;
    expect(download !== null || response !== null).toBeTruthy();
  });
});
