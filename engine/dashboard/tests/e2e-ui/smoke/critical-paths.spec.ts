/**
 * E2E-UI Smoke Tests — Critical User Paths (Real Server)
 *
 * These tests run against a REAL Flapjack server — NO mocking whatsoever.
 * The `e2e-products` index is pre-seeded with 12 products, synonyms, rules,
 * and analytics data via seed.setup.ts.
 *
 * Prerequisites:
 * - Flapjack server running on port 7700
 * - Vite dev server on port 5177 (proxying API to 7700)
 * - Auth pre-seeded via auth fixture (localStorage)
 *
 * Per AI_TESTING_METHODOLOGY.md:
 * - Smoke tests cover 7 critical paths (~2 min total)
 * - Run on every commit (CI)
 * - Catch: navigation bugs, layout issues, integration failures
 * - No hardcoded sleeps — use Playwright auto-waiting
 */
import { test, expect } from '../../fixtures/auth.fixture';

const TEST_INDEX = 'e2e-products';
const TEMP_INDEX = 'e2e-temp';

test.describe('Smoke Tests', () => {
  // ===========================================================================
  // SMOKE 1: Overview loads with real data
  // ===========================================================================
  test('Overview loads with real data', async ({ page }) => {
    await page.goto('/overview');

    // Verify the page heading renders
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible();

    // Verify "e2e-products" appears in the index list
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible();

    // Verify health status shows "Healthy"
    const statusCard = page.getByTestId('stat-card-status');
    await expect(statusCard.getByText('Healthy')).toBeVisible();

    // Verify stat cards are visible and populated
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible();
    await expect(page.getByTestId('stat-card-documents')).toBeVisible();
    await expect(page.getByTestId('stat-card-storage')).toBeVisible();

    // The indexes stat should show at least 1 (the seeded index)
    const indexesCard = page.getByTestId('stat-card-indexes');
    await expect(indexesCard.locator('.text-2xl')).not.toHaveText('0');
  });

  // ===========================================================================
  // SMOKE 2: Search returns real results
  // ===========================================================================
  test('Search returns real results', async ({ page }) => {
    await page.goto(`/index/${TEST_INDEX}`);

    // Verify the index heading renders
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible();

    // Search input should be visible
    const searchInput = page.getByPlaceholder(/search documents/i);
    await expect(searchInput).toBeVisible();

    // Type "laptop" and press Enter
    await searchInput.fill('laptop');
    await searchInput.press('Enter');

    // Verify results appear — seeded data includes products matching "laptop"
    const resultsPanel = page.locator('[data-testid="results-panel"]');
    await expect(resultsPanel).toBeVisible({ timeout: 10000 });

    // Verify at least one document card rendered with real data
    await expect(resultsPanel.locator('[data-testid="document-card"]').first()).toBeVisible({ timeout: 10000 });

    // Verify result count text shows "results" somewhere in the panel
    await expect(resultsPanel.getByText('results').first()).toBeVisible();
  });

  // ===========================================================================
  // SMOKE 3: Sidebar navigation works
  // ===========================================================================
  test('Sidebar navigation works', async ({ page }) => {
    await page.goto('/overview');

    // Verify we start at overview
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible();

    const sidebar = page.locator('aside');

    // Navigate to API Keys
    await sidebar.getByRole('link', { name: /api keys/i }).click();
    await expect(page).toHaveURL(/\/keys/);
    await expect(page.getByText(/api keys/i).first()).toBeVisible();

    // Navigate to System
    await sidebar.getByRole('link', { name: /system/i }).click();
    await expect(page).toHaveURL(/\/system/);
    await expect(page.getByRole('heading', { name: /system/i })).toBeVisible();

    // Navigate back to Overview
    await sidebar.getByRole('link', { name: /overview/i }).click();
    await expect(page).toHaveURL(/\/overview/);
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible();
  });

  // ===========================================================================
  // SMOKE 4: Settings page loads
  // ===========================================================================
  test('Settings page loads with searchable attributes', async ({ page }) => {
    await page.goto(`/index/${TEST_INDEX}/settings`);

    // Settings heading should be visible
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible();

    // Breadcrumb should show the index name
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible();

    // Searchable attributes from the seeded SETTINGS should be visible:
    // ['name', 'description', 'brand', 'category', 'tags']
    await expect(page.getByText('name').first()).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('description').first()).toBeVisible();
    await expect(page.getByText('brand').first()).toBeVisible();
    await expect(page.getByText('category').first()).toBeVisible();
  });

  // ===========================================================================
  // SMOKE 5: API Keys page loads
  // ===========================================================================
  test('API Keys page loads', async ({ page }) => {
    await page.goto('/keys');

    // Heading should be visible
    await expect(page.getByText(/api keys/i).first()).toBeVisible();

    // The Create Key button should be visible (either the header button or empty-state CTA)
    await expect(page.getByRole('button', { name: 'Create Key' }).or(page.getByRole('button', { name: 'Create Your First Key' })).first()).toBeVisible();

    // The keys list section should be visible (either key cards or empty state)
    // The seeded admin key (fj_devtestadminkey000000) should have created at least
    // one key entry, or there should be a "no keys" message with a create prompt
    const keysList = page.getByTestId('keys-list');
    const emptyState = page.getByText(/no.*key/i);
    await expect(keysList.or(emptyState).first()).toBeVisible({ timeout: 10000 });
  });

  // ===========================================================================
  // SMOKE 6: System health displays
  // ===========================================================================
  test('System health displays', async ({ page }) => {
    await page.goto('/system');

    // System heading should be visible
    await expect(page.getByRole('heading', { name: /system/i })).toBeVisible();

    // Health tab should be visible by default
    await expect(page.getByText(/health/i).first()).toBeVisible();

    // The health status card should show "ok" (the real server status)
    const healthStatusCard = page.getByTestId('health-status');
    await expect(healthStatusCard.getByText('ok')).toBeVisible({ timeout: 10000 });

    // Index health summary should show the seeded index
    const indexHealth = page.getByTestId('index-health-summary');
    await expect(indexHealth).toBeVisible();
    await expect(indexHealth.getByText(TEST_INDEX)).toBeVisible();
  });

  // ===========================================================================
  // SMOKE 7: Create and delete index
  // ===========================================================================
  test('Create and delete index', async ({ page }) => {
    // Clean up the temp index first in case a previous test run left it
    // (the cleanup.setup.ts handles this too, but be safe)
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible();

    try {
      // Open create dialog
      await page.getByRole('button', { name: /create.*index/i }).click();
      const dialog = page.getByRole('dialog');
      await expect(dialog).toBeVisible();
      await expect(dialog.getByRole('heading', { name: 'Create Index' })).toBeVisible();

      // Fill in "e2e-temp" as the index name
      await dialog.locator('#index-uid').fill(TEMP_INDEX);

      // Submit the form — click the "Create Index" button
      await dialog.getByRole('button', { name: /create index/i }).click();

      // Wait for dialog to close (indicates success)
      await expect(dialog).not.toBeVisible({ timeout: 15000 });

      // Verify "e2e-temp" appears somewhere (sidebar or index list)
      await expect(page.getByText(TEMP_INDEX).first()).toBeVisible({ timeout: 10000 });

      // The index list is paginated (10 per page). Navigate to the page containing the new index.
      const deleteBtn = page.getByTitle(`Delete index "${TEMP_INDEX}"`);
      while (await deleteBtn.count() === 0) {
        const nextBtn = page.getByRole('button', { name: /next/i });
        if (await nextBtn.isEnabled()) {
          await nextBtn.click();
          await page.waitForTimeout(500);
        } else {
          break; // no more pages
        }
      }
      await deleteBtn.click();

      // Confirm the deletion dialog
      const confirmDialog = page.getByRole('dialog');
      await expect(confirmDialog).toBeVisible();
      await expect(confirmDialog.getByText(TEMP_INDEX)).toBeVisible();

      // Click the destructive "Delete" button to confirm
      await confirmDialog.getByRole('button', { name: /^delete$/i }).click();

      // Wait for the dialog to close
      await expect(confirmDialog).not.toBeVisible({ timeout: 10000 });

      // Verify "e2e-temp" is gone from the sidebar index list
      // (a toast notification may still mention it, so scope to sidebar)
      const sidebar = page.locator('aside');
      await expect(sidebar.getByText(TEMP_INDEX)).not.toBeVisible({ timeout: 10000 });
    } finally {
      // Cleanup: ensure e2e-temp is deleted even if the test fails mid-way
      // Use the API directly for reliable cleanup
      try {
        await page.request.delete(
          `http://localhost:7700/1/indexes/${TEMP_INDEX}`,
          {
            headers: {
              'x-algolia-application-id': 'flapjack',
              'x-algolia-api-key': 'fj_devtestadminkey000000',
              'Content-Type': 'application/json',
            },
          }
        );
        // Ignore errors — the index may already be deleted
      } catch {
        // Cleanup is best-effort
      }
    }
  });
});
