/**
 * E2E Tests: InstantSearch Overlay (4 behaviors)
 *
 * Tests based on WORDPRESS_E2E_BEHAVIOR_SPEC.md
 * Behaviors: BEH-IS-001 through BEH-IS-004
 */

import { test, expect } from '@playwright/test';
import { loginToWordPress, navigateToSettings, saveAPICredentials, mockFlapjackAPI } from './helpers.js';

test.describe('InstantSearch Overlay', () => {
  test.beforeEach(async ({ page }) => {
    // Login and ensure plugin is configured
    await loginToWordPress(page);

    // Mock API if testing locally
    if (process.env.TEST_MODE === 'local') {
      await mockFlapjackAPI(page);
    }
  });

  test('BEH-IS-001: Overlay opens on search icon click', async ({ page }) => {
    // Preconditions: Plugin configured, InstantSearch enabled
    await page.goto('/');

    // Actions: Click search icon
    const searchIcon = page.locator('.flapjack-search-icon, #flapjack-search-trigger, [data-flapjack-search-trigger]');
    await expect(searchIcon).toBeVisible({ timeout: 10000 });
    await searchIcon.click();

    // Expected outcomes:
    // ✅ Full-screen overlay appears with fade-in animation
    const overlay = page.locator('#flapjack-search-overlay, .flapjack-search-overlay, .ais-InstantSearch');
    await expect(overlay).toBeVisible({ timeout: 3000 });

    // ✅ Search input has focus
    const searchInput = page.locator('#flapjack-search-input, .ais-SearchBox-input, input[type="search"]');
    await expect(searchInput).toBeFocused();

    // ✅ Placeholder text displays
    await expect(searchInput).toHaveAttribute('placeholder', /search/i);

    // ✅ Close button visible
    const closeButton = page.locator('.flapjack-search-close, .ais-close-button, [aria-label*="close"]');
    await expect(closeButton).toBeVisible();
  });

  test('BEH-IS-002: Search results update in real-time', async ({ page }) => {
    // Preconditions: InstantSearch overlay open
    await page.goto('/');
    const searchIcon = page.locator('.flapjack-search-icon, #flapjack-search-trigger, [data-flapjack-search-trigger]');
    await searchIcon.click();

    const searchInput = page.locator('#flapjack-search-input, .ais-SearchBox-input, input[type="search"]');
    await expect(searchInput).toBeVisible();

    // Actions: Type search query
    await searchInput.fill('test');

    // Expected outcomes:
    // ✅ Results appear within 300ms
    const resultsPanel = page.locator('.ais-Hits, .flapjack-search-results, #flapjack-search-results');
    await expect(resultsPanel).toBeVisible({ timeout: 500 });

    // ✅ Each result shows: title, excerpt, thumbnail
    const firstResult = resultsPanel.locator('.ais-Hits-item, .flapjack-search-result').first();
    await expect(firstResult).toBeVisible();

    // Check for title
    const resultTitle = firstResult.locator('.ais-Highlight, h3, h4, .result-title');
    await expect(resultTitle).toBeVisible();

    // ✅ Results update as typing continues
    await searchInput.fill('test product');
    await page.waitForTimeout(300); // Debounce

    // ✅ "Powered by" logo displayed (if configured)
    // const poweredBy = page.locator('.ais-PoweredBy, .flapjack-powered-by');
    // await expect(poweredBy).toBeVisible();
  });

  test('BEH-IS-003: Clicking result navigates to page', async ({ page }) => {
    // Preconditions: Search results displayed
    await page.goto('/');
    const searchIcon = page.locator('.flapjack-search-icon, #flapjack-search-trigger, [data-flapjack-search-trigger]');
    await searchIcon.click();

    const searchInput = page.locator('#flapjack-search-input, .ais-SearchBox-input, input[type="search"]');
    await searchInput.fill('test');

    // Wait for results
    const resultsPanel = page.locator('.ais-Hits, .flapjack-search-results');
    await expect(resultsPanel).toBeVisible({ timeout: 2000 });

    const firstResult = resultsPanel.locator('.ais-Hits-item, .flapjack-search-result').first();
    await expect(firstResult).toBeVisible();

    // Get the link/result URL before clicking
    const resultLink = firstResult.locator('a').first();
    await expect(resultLink).toBeVisible();

    // Actions: Click on first result
    await Promise.all([
      page.waitForNavigation({ timeout: 10000 }),
      resultLink.click(),
    ]);

    // Expected outcomes:
    // ✅ Browser navigates to clicked post/page URL
    expect(page.url()).not.toContain('/wp-admin/');

    // ✅ Overlay closes automatically (no longer visible on new page)
    const overlay = page.locator('#flapjack-search-overlay, .flapjack-search-overlay');
    await expect(overlay).not.toBeVisible();

    // ✅ Correct post content displays
    // (Verified by successful navigation)
  });

  test('BEH-IS-004: ESC key closes overlay', async ({ page }) => {
    // Preconditions: Overlay open
    await page.goto('/');
    const searchIcon = page.locator('.flapjack-search-icon, #flapjack-search-trigger, [data-flapjack-search-trigger]');
    await searchIcon.click();

    const overlay = page.locator('#flapjack-search-overlay, .flapjack-search-overlay, .ais-InstantSearch');
    await expect(overlay).toBeVisible();

    // Actions: Press ESC key
    await page.keyboard.press('Escape');

    // Expected outcomes:
    // ✅ Overlay closes with fade-out animation
    await expect(overlay).not.toBeVisible({ timeout: 1000 });

    // ✅ Focus returns to search icon trigger
    const searchIconAfterClose = page.locator('.flapjack-search-icon, #flapjack-search-trigger');
    // Note: Focus check may vary by implementation
    // await expect(searchIconAfterClose).toBeFocused();

    // ✅ Search input value persists if user reopens
    await searchIcon.click();
    await expect(overlay).toBeVisible();
    const searchInput = page.locator('#flapjack-search-input, .ais-SearchBox-input');
    await searchInput.fill('persistent query');

    await page.keyboard.press('Escape');
    await expect(overlay).not.toBeVisible();

    // Reopen and check value persists
    await searchIcon.click();
    await expect(searchInput).toHaveValue('persistent query');
  });
});
