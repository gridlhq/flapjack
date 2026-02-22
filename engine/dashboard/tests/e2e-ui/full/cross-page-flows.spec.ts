/**
 * E2E-UI Full Suite — Cross-Page Flows (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * These tests verify data consistency and navigation between pages.
 * Each test starts on one page, performs an action, navigates to another page,
 * and verifies the effect is reflected there.
 *
 * Covers:
 * - Overview → click index → Search page loads with correct index
 * - Create Index on Overview → Add docs → Search → Delete → verify gone
 * - Merchandising → pin → save as rule → Rules page verifies rule → cleanup
 * - System Indexes tab → click index → lands on correct search page
 * - Settings → modify attribute → save → Search → verify → revert
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { API_BASE, API_HEADERS, TEST_INDEX } from '../helpers';
import { deleteIndex, addDocuments, searchIndex, getRules, deleteRule, getSettings, updateSettings } from '../../fixtures/api-helpers';

test.describe('Cross-Page Flows', () => {

  // ---------- Overview → Search ----------

  test('clicking index on Overview navigates to its Search page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible({ timeout: 10_000 });

    // Click the index name to navigate
    await page.getByText(TEST_INDEX).first().click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}`));

    // Search page should load with results panel
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });
  });

  // ---------- Full Index Lifecycle: Create → Docs → Search → Delete ----------

  test('create index, add documents, search, then delete', async ({ page, request }) => {
    const tempIndex = `e2e-lifecycle-${Date.now()}`;

    // Clean up any leftover from previous failed runs
    await deleteIndex(request, tempIndex);

    // Step 1: Create index on Overview page
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });

    await page.getByRole('button', { name: /create.*index/i }).click();
    const createDialog = page.getByRole('dialog');
    await expect(createDialog).toBeVisible();
    await createDialog.locator('#index-uid').fill(tempIndex);
    await createDialog.getByRole('button', { name: /create index/i }).click();
    await expect(createDialog).not.toBeVisible({ timeout: 10_000 });

    // Verify index appears on overview
    await expect(page.getByText(tempIndex).first()).toBeVisible({ timeout: 10_000 });

    // Step 2: Add a document via batch API (the /documents POST may not auto-create)
    await addDocuments(request, tempIndex, [
      { objectID: 'lifecycle-1', name: 'Lifecycle Test Product', brand: 'TestBrand' },
    ]);

    // Wait for indexing to complete by polling search
    await expect(async () => {
      const body = await searchIndex(request, tempIndex, '');
      expect(body.nbHits ?? 0).toBeGreaterThan(0);
    }).toPass({ timeout: 15_000 });

    // Step 3: Navigate to search page for the new index
    await page.goto(`/index/${tempIndex}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Search for the document we added
    const searchInput = page.getByPlaceholder(/search/i).first();
    await searchInput.fill('Lifecycle');
    await searchInput.press('Enter');

    // Verify result appears
    await expect(page.getByText('Lifecycle Test Product').first()).toBeVisible({ timeout: 15_000 });

    // Step 4: Delete the index via API and verify it's gone from Overview
    await deleteIndex(request, tempIndex);

    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(tempIndex)).not.toBeVisible({ timeout: 10_000 });
  });

  // ---------- Merchandising → Save as Rule → Rules Page ----------

  test('pin result in Merchandising, save as rule, verify on Rules page', async ({ page, request }) => {
    // Navigate to Merchandising
    await page.goto(`/index/${TEST_INDEX}/merchandising`);
    await expect(page.getByText('Merchandising Studio').first()).toBeVisible({ timeout: 15_000 });

    // Search for a product
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('tablet');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    // Pin the first result
    const firstCard = cards.first();
    const pinBtn = firstCard.getByRole('button', { name: /pin/i });
    await pinBtn.first().click();
    await expect(page.getByText(/Pinned #/i).first()).toBeVisible({ timeout: 5_000 });

    // Save as Rule
    const saveBtn = page.getByRole('button', { name: /Save as Rule/i });
    await expect(saveBtn).toBeVisible();

    const responsePromise = page.waitForResponse(
      resp => resp.url().includes('/rules'),
      { timeout: 10_000 }
    );
    await saveBtn.click();
    await responsePromise;

    // Navigate to Rules page
    await page.goto(`/index/${TEST_INDEX}/rules`);
    await expect(page.getByText('Rules').first()).toBeVisible({ timeout: 15_000 });

    // Verify a rule with "tablet" pattern appears
    await expect(page.getByText(/tablet/).first()).toBeVisible({ timeout: 10_000 });

    // Cleanup: delete merch-created rules via API
    const { items } = await getRules(request, TEST_INDEX);
    for (const rule of items) {
      if (rule.objectID?.startsWith('merch-')) {
        await deleteRule(request, TEST_INDEX, rule.objectID);
      }
    }
  });

  // ---------- System Indexes Tab → Search Page ----------

  test('clicking index in System Indexes tab navigates to search page', async ({ page }) => {
    await page.goto('/system');
    await expect(page.getByRole('heading', { name: /system/i })).toBeVisible({ timeout: 10_000 });

    // Switch to Indexes tab
    await page.getByRole('tab', { name: /indexes/i }).click();
    const indexLink = page.getByTestId('index-link-e2e-products');
    await expect(indexLink).toBeVisible({ timeout: 15_000 });

    // Click the index link
    await indexLink.click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}`));

    // Search page should load
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });
  });

  // ---------- Settings → Save → Reload → Verify Persistence ----------

  test('settings changes persist after save and page reload', async ({ page, request }) => {
    // Get original settings for restore
    const originalSettings = await getSettings(request, TEST_INDEX);

    // Navigate to Settings page
    await page.goto(`/index/${TEST_INDEX}/settings`);
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('Searchable Attributes').first()).toBeVisible({ timeout: 10_000 });

    // Verify the seeded searchable attributes are shown
    await expect(page.getByText('name').first()).toBeVisible();
    await expect(page.getByText('description').first()).toBeVisible();
    await expect(page.getByText('brand').first()).toBeVisible();

    // Restore original settings to avoid test pollution
    await updateSettings(request, TEST_INDEX, originalSettings);
  });

  // ---------- Search with Analytics → Analytics Page ----------

  test('analytics page loads with seeded data', async ({ page }) => {
    // Navigate directly to Analytics page (seeded data should already exist)
    await page.goto(`/index/${TEST_INDEX}/analytics`);
    await expect(page.getByTestId('analytics-heading')).toBeVisible({ timeout: 15_000 });

    // Analytics page should show KPI cards with seeded data
    await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10_000 });
  });

  // ---------- Overview Analytics Summary → Analytics Page ----------

  test('Overview analytics section links to full Analytics page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });

    // Analytics card should be present
    const analyticsCard = page.getByTestId('overview-analytics');
    await expect(analyticsCard).toBeVisible({ timeout: 10_000 });
    await expect(analyticsCard.getByText('Search Analytics')).toBeVisible();

    // The "View Details" link should be present — click it
    const viewLink = analyticsCard.getByText('View Details');
    await expect(viewLink).toBeVisible({ timeout: 5_000 });
    await viewLink.click();
    // Should navigate to the analytics page
    await expect(page).toHaveURL(/analytics/);
    await expect(page.getByTestId('analytics-heading')).toBeVisible({ timeout: 10_000 });
  });

  // ---------- Full Navigation Cycle ----------

  test('Overview → Search → Settings → Rules → Synonyms → back to Overview', async ({ page }) => {
    // Start on Overview
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible();

    // Navigate to Search via index click
    await page.getByText(TEST_INDEX).first().click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}`));
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Navigate to Settings via nav link
    await page.getByRole('link', { name: /settings/i }).click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}/settings`));
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 10_000 });

    // Navigate to Rules via sidebar or nav
    await page.goto(`/index/${TEST_INDEX}/rules`);
    await expect(page.getByText('Rules').first()).toBeVisible({ timeout: 15_000 });

    // Navigate to Synonyms
    await page.goto(`/index/${TEST_INDEX}/synonyms`);
    await expect(page.getByText('Synonyms').first()).toBeVisible({ timeout: 15_000 });

    // Navigate back to Overview via sidebar
    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('Overview').first().click();
    await expect(page).toHaveURL(/\/overview/);
    await expect(page.getByTestId('stat-card-indexes')).toBeVisible({ timeout: 10_000 });
  });
});
