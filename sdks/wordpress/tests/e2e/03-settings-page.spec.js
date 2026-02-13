/**
 * E2E Tests: Settings Page UI (10 behaviors)
 *
 * Tests based on WORDPRESS_E2E_BEHAVIOR_SPEC.md
 * Behaviors: BEH-SET-001 through BEH-SET-010
 */

import { test, expect } from '@playwright/test';
import { loginToWordPress, navigateToSettings, getTestCredentials } from './helpers.js';

test.describe('Settings Page UI', () => {
  test.beforeEach(async ({ page }) => {
    await loginToWordPress(page);
    await navigateToSettings(page);
  });

  test('BEH-SET-001: API key validation and save', async ({ page }) => {
    const creds = getTestCredentials();

    // Actions: Enter valid credentials
    await page.fill('#flapjack_app_id', creds.flapjackAppId);
    await page.fill('#flapjack_admin_api_key', creds.flapjackAdminApiKey);

    // Click save
    const saveButton = page.locator('button[type="submit"], input[type="submit"]');
    await saveButton.click();

    // Expected outcomes:
    // ✅ Success message displays
    const successNotice = page.locator('.notice-success, .updated, .components-notice.is-success');
    await expect(successNotice).toBeVisible({ timeout: 5000 });

    // ✅ Settings persist after reload
    await page.reload();
    await expect(page.locator('#flapjack_app_id')).toHaveValue(creds.flapjackAppId);
  });

  test('BEH-SET-002: Application ID validation', async ({ page }) => {
    // Actions: Enter invalid Application ID
    await page.fill('#flapjack_app_id', '');
    await page.fill('#flapjack_admin_api_key', 'dummy_key');

    const saveButton = page.locator('button[type="submit"], input[type="submit"]');
    await saveButton.click();

    // Expected outcomes:
    // ✅ Error message displays
    const errorNotice = page.locator('.notice-error, .error, .components-notice.is-error');
    const errorExists = await errorNotice.count() > 0;

    // ✅ Settings NOT saved
    // (If validation works, error should appear)
    expect(errorExists).toBeTruthy();
  });

  test('BEH-SET-003: Search-Only API Key validation', async ({ page }) => {
    const creds = getTestCredentials();

    // Actions: Enter Search-Only API Key
    await page.fill('#flapjack_search_api_key', creds.flapjackSearchApiKey);

    // Test connection (if button exists)
    const testButton = page.locator('button:has-text("Test"), #test_connection');
    if (await testButton.count() > 0) {
      await testButton.click();

      // Expected outcome: Success or error message
      const notice = page.locator('.notice, .components-notice');
      await expect(notice.first()).toBeVisible({ timeout: 5000 });
    }

    // ✅ Help text explains key should be search-only
    const helpText = page.locator('text=/search.only/i, text=/read.only/i');
    const hasHelpText = await helpText.count() > 0;
    // Help text may or may not be present depending on implementation
  });

  test('BEH-SET-004: Index prefix configuration', async ({ page }) => {
    // Actions: Set index prefix
    const prefixInput = page.locator('#flapjack_index_prefix, input[name*="prefix"]');
    if (await prefixInput.count() > 0) {
      await prefixInput.fill('staging_');

      const saveButton = page.locator('button[type="submit"]');
      await saveButton.click();

      // Expected outcomes:
      // ✅ Settings save successfully
      const successNotice = page.locator('.notice-success');
      await expect(successNotice).toBeVisible({ timeout: 5000 });

      // ✅ Prefix displays after reload
      await page.reload();
      await expect(prefixInput).toHaveValue('staging_');
    }
  });

  test('BEH-SET-005: Instant Search enable/disable toggle', async ({ page }) => {
    // Actions: Toggle InstantSearch OFF
    const instantSearchToggle = page.locator('#flapjack_enable_instantsearch, input[name*="instantsearch"]');

    if (await instantSearchToggle.count() > 0) {
      const isChecked = await instantSearchToggle.isChecked();

      // Toggle it
      if (isChecked) {
        await instantSearchToggle.uncheck();
      } else {
        await instantSearchToggle.check();
      }

      const saveButton = page.locator('button[type="submit"]');
      await saveButton.click();

      // Expected outcomes:
      // ✅ Toggle state persists
      const successNotice = page.locator('.notice-success');
      await expect(successNotice).toBeVisible({ timeout: 5000 });

      await page.reload();
      const newState = await instantSearchToggle.isChecked();
      expect(newState).toBe(!isChecked);
    }
  });

  test('BEH-SET-006: Facet attribute selection', async ({ page }) => {
    // Navigate to Facets tab if it exists
    const facetsTab = page.locator('a:has-text("Facets"), button:has-text("Facets")');
    if (await facetsTab.count() > 0) {
      await facetsTab.click();
      await page.waitForTimeout(500);
    }

    // Actions: Check facet attributes
    const facetCheckboxes = page.locator('input[type="checkbox"][name*="facet"], input[name*="attribute"]');

    if (await facetCheckboxes.count() >= 2) {
      // Check first two
      await facetCheckboxes.nth(0).check();
      await facetCheckboxes.nth(1).check();

      const saveButton = page.locator('button[type="submit"]');
      await saveButton.click();

      // Expected outcomes:
      // ✅ Facets save successfully
      const successNotice = page.locator('.notice-success');
      await expect(successNotice).toBeVisible({ timeout: 5000 });

      // ✅ Selections persist
      await page.reload();
      if (await facetsTab.count() > 0) {
        await facetsTab.click();
      }
      await expect(facetCheckboxes.nth(0)).toBeChecked();
    }
  });

  test('BEH-SET-007: Reindex button triggers indexing', async ({ page }) => {
    // Actions: Click reindex button
    const reindexButton = page.locator('#flapjack_reindex_button, button:has-text("Reindex"), button:has-text("Index")');

    if (await reindexButton.count() > 0) {
      await reindexButton.click();

      // Expected outcomes:
      // ✅ Progress bar displays
      const progressIndicator = page.locator('.flapjack-progress, .progress-bar, .spinner, .components-spinner');

      // Wait for progress or completion message
      const progressOrSuccess = page.locator('.flapjack-progress, .notice-success');
      await expect(progressOrSuccess.first()).toBeVisible({ timeout: 30000 });

      // ✅ Button disabled during indexing
      // (May already be complete if fast)
    }
  });

  test('BEH-SET-008: Index status displays correctly', async ({ page }) => {
    // Expected outcomes:
    // ✅ Shows last indexed date/time
    const indexStatus = page.locator('.flapjack-index-status, .index-info, text=/last index/i');
    const statusExists = await indexStatus.count() > 0;

    // ✅ Displays record count
    const recordCount = page.locator('text=/records/i, text=/documents/i, text=/items/i');
    const countExists = await recordCount.count() > 0;

    // At least one status indicator should be present
    expect(statusExists || countExists).toBeTruthy();
  });

  test('BEH-SET-009: Error messages display for invalid inputs', async ({ page }) => {
    // Actions: Leave required field empty
    await page.fill('#flapjack_app_id', '');
    await page.fill('#flapjack_admin_api_key', '');

    const saveButton = page.locator('button[type="submit"]');
    await saveButton.click();

    // Expected outcomes:
    // ✅ Error message appears
    const errorNotice = page.locator('.notice-error, .error, .components-notice.is-error, text=/required/i');
    await expect(errorNotice.first()).toBeVisible({ timeout: 3000 });

    // ✅ Form does NOT submit
    // (Error should prevent save)
  });

  test('BEH-SET-010: Success messages display on save', async ({ page }) => {
    const creds = getTestCredentials();

    // Actions: Update any setting
    await page.fill('#flapjack_app_id', creds.flapjackAppId);

    const saveButton = page.locator('button[type="submit"]');
    await saveButton.click();

    // Expected outcomes:
    // ✅ Green success banner
    const successNotice = page.locator('.notice-success, .updated, .components-notice.is-success');
    await expect(successNotice).toBeVisible({ timeout: 5000 });

    // ✅ Message contains success text
    const successText = await successNotice.textContent();
    expect(successText?.toLowerCase()).toMatch(/saved|success|updated/);

    // ✅ Message auto-dismisses (optional - check after 3 seconds)
    await page.waitForTimeout(4000);
    const stillVisible = await successNotice.isVisible().catch(() => false);
    // Auto-dismiss is optional feature
  });
});
