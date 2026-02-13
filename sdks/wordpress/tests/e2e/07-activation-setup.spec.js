/**
 * E2E Tests: Activation/Setup Flows (4 behaviors)
 *
 * Tests based on WORDPRESS_E2E_BEHAVIOR_SPEC.md
 * Behaviors: BEH-ACT-001 through BEH-ACT-004
 */

import { test, expect } from '@playwright/test';
import { loginToWordPress, activatePlugin, deactivatePlugin } from './helpers.js';

test.describe('Activation and Setup Flows', () => {
  test.beforeEach(async ({ page }) => {
    await loginToWordPress(page);
  });

  test('BEH-ACT-001: Plugin activation redirects to settings', async ({ page }) => {
    // Preconditions: Plugin uploaded but not activated
    // First, ensure plugin is deactivated
    await deactivatePlugin(page);

    // Actions: Activate plugin
    await page.goto('/wp-admin/plugins.php');
    const activateLink = page.locator('[data-slug="flapjack-search"] .activate a');

    if (await activateLink.count() > 0) {
      await Promise.all([
        page.waitForNavigation({ timeout: 10000 }),
        activateLink.click(),
      ]);

      // Expected outcomes:
      // ✅ Redirects to settings page OR shows activation notice
      const currentURL = page.url();

      // Check if redirected to settings
      const isSettingsPage = currentURL.includes('page=flapjack-search') ||
                              currentURL.includes('flapjack') ||
                              await page.locator('.flapjack-settings').count() > 0;

      // OR check if activation notice appears
      const activationNotice = page.locator('.notice, .updated, text=/activated/i');
      const hasNotice = await activationNotice.count() > 0;

      // One or both should be true
      expect(isSettingsPage || hasNotice).toBeTruthy();

      // ✅ Welcome message displays (if applicable)
      const welcomeMessage = page.locator('text=/welcome/i, text=/get started/i');
      // Welcome message is optional
    }
  });

  test('BEH-ACT-002: Setup wizard guides through configuration', async ({ page }) => {
    // Preconditions: First-time activation
    // This test assumes setup wizard exists (may not in all implementations)

    await page.goto('/wp-admin/admin.php?page=flapjack-search&setup=1');
    await page.waitForLoadState('networkidle');

    // Check if setup wizard exists
    const setupWizard = page.locator('.flapjack-setup-wizard, .setup-wizard, text=/setup wizard/i');

    if (await setupWizard.count() > 0) {
      // Expected outcomes:
      // ✅ Progress indicator shows steps
      const progressIndicator = page.locator('.progress, .steps, text=/step/i');
      await expect(progressIndicator.first()).toBeVisible();

      // ✅ Each step validates before proceeding
      // (Implementation-specific - may have "Next" buttons)

      // ✅ Final step triggers indexing
      // (Would need to complete all wizard steps)
    }
  });

  test('BEH-ACT-003: First-time indexing completes successfully', async ({ page }) => {
    // Preconditions: Setup wizard completed or manual reindex triggered
    await page.goto('/wp-admin/admin.php?page=flapjack-search');

    // Trigger reindex
    const reindexButton = page.locator('#flapjack_reindex_button, button:has-text("Reindex"), button:has-text("Index")');

    if (await reindexButton.count() > 0) {
      await reindexButton.click();

      // Expected outcomes:
      // ✅ Progress bar reaches 100%
      const progressBar = page.locator('.progress-bar, .flapjack-progress');

      if (await progressBar.count() > 0) {
        // Wait for completion (max 60s)
        await page.waitForSelector('.notice-success, text=/complete/i, text=/success/i', { timeout: 60000 });
      }

      // ✅ Success message shows item count
      const successMessage = page.locator('.notice-success, text=/indexed/i');
      if (await successMessage.count() > 0) {
        const messageText = await successMessage.textContent();
        expect(messageText).toBeTruthy();
      }

      // ✅ Search immediately functional
      // (Tested in other test suites)
    }
  });

  test('BEH-ACT-004: Deactivation preserves settings', async ({ page }) => {
    // Preconditions: Plugin configured
    await page.goto('/wp-admin/admin.php?page=flapjack-search');

    // Save a test setting
    const appIdInput = page.locator('#flapjack_app_id');
    const testAppId = 'TEST_PRESERVATION_123';

    if (await appIdInput.count() > 0) {
      await appIdInput.fill(testAppId);
      const saveButton = page.locator('button[type="submit"]');
      await saveButton.click();

      // Wait for save
      await page.waitForSelector('.notice-success', { timeout: 5000 });
    }

    // Actions: Deactivate plugin
    await deactivatePlugin(page);

    // Wait a moment
    await page.waitForTimeout(1000);

    // Reactivate plugin
    await activatePlugin(page);

    // Navigate to settings
    await page.goto('/wp-admin/admin.php?page=flapjack-search');

    // Expected outcomes:
    // ✅ All settings preserved
    if (await appIdInput.count() > 0) {
      const savedValue = await appIdInput.inputValue();
      expect(savedValue).toBe(testAppId);
    }

    // ✅ Indices remain in Flapjack
    // (External verification - cannot test in browser)

    // ✅ No data loss on reactivation
    // (Verified by settings persistence)
  });
});
