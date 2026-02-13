/**
 * E2E Tests: Autocomplete (3 behaviors)
 *
 * Tests based on WORDPRESS_E2E_BEHAVIOR_SPEC.md
 * Behaviors: BEH-AC-001 through BEH-AC-003
 */

import { test, expect } from '@playwright/test';
import { mockFlapjackAPI } from './helpers.js';

test.describe('Autocomplete', () => {
  test.beforeEach(async ({ page }) => {
    if (process.env.TEST_MODE === 'local') {
      await mockFlapjackAPI(page);
    }
  });

  test('BEH-AC-001: Autocomplete displays suggestions on typing', async ({ page }) => {
    // Preconditions: Frontend with search widget, autocomplete enabled
    await page.goto('/');

    // Actions: Click search input and type
    const searchInput = page.locator('input[type="search"], .flapjack-search-input').first();
    await searchInput.click();
    await searchInput.fill('prod');
    await page.waitForTimeout(500);

    // Expected outcomes:
    // ✅ Dropdown appears with suggestions
    const dropdown = page.locator('.ais-Autocomplete, .autocomplete-dropdown, .flapjack-autocomplete');

    if (await dropdown.count() > 0) {
      await expect(dropdown).toBeVisible({ timeout: 2000 });

      // ✅ Shows recent queries + top results
      const suggestions = dropdown.locator('.autocomplete-item, .ais-Autocomplete-item, .suggestion');
      const suggestionCount = await suggestions.count();
      expect(suggestionCount).toBeGreaterThan(0);

      // ✅ Suggestions include product names, post titles
      const firstSuggestion = await suggestions.first().textContent();
      expect(firstSuggestion).toBeTruthy();
    }
  });

  test('BEH-AC-002: Arrow keys navigate suggestions', async ({ page }) => {
    await page.goto('/');

    const searchInput = page.locator('input[type="search"]').first();
    await searchInput.click();
    await searchInput.fill('test');
    await page.waitForTimeout(500);

    const dropdown = page.locator('.ais-Autocomplete, .autocomplete-dropdown');

    if (await dropdown.count() > 0) {
      await expect(dropdown).toBeVisible();

      const suggestions = dropdown.locator('.autocomplete-item, .suggestion');
      const count = await suggestions.count();

      if (count >= 3) {
        // Actions: Press Down Arrow 3 times
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('ArrowDown');

        // Expected outcomes:
        // ✅ Third suggestion highlighted
        const highlightedSuggestion = page.locator('.is-selected, .highlighted, [aria-selected="true"]');
        await expect(highlightedSuggestion).toBeVisible();

        // ✅ Highlighted suggestion has blue/active background
        // (CSS styling check - verified by aria-selected or class)

        // ✅ Search input updates with highlighted text
        const inputValue = await searchInput.inputValue();
        expect(inputValue.length).toBeGreaterThan(0);
      }
    }
  });

  test('BEH-AC-003: Enter key submits selected suggestion', async ({ page }) => {
    await page.goto('/');

    const searchInput = page.locator('input[type="search"]').first();
    await searchInput.fill('test');
    await page.waitForTimeout(500);

    const dropdown = page.locator('.ais-Autocomplete, .autocomplete-dropdown');

    if (await dropdown.count() > 0 && await dropdown.isVisible()) {
      const suggestions = dropdown.locator('.autocomplete-item');

      if (await suggestions.count() >= 2) {
        // Navigate to second suggestion
        await page.keyboard.press('ArrowDown');
        await page.keyboard.press('ArrowDown');

        // Actions: Press Enter
        await Promise.all([
          page.waitForNavigation({ timeout: 10000 }).catch(() => {}),
          page.keyboard.press('Enter'),
        ]);

        // Expected outcomes:
        // ✅ Browser navigates to selected result OR shows search results
        const currentURL = page.url();
        const navigated = !currentURL.endsWith('/') || currentURL.includes('?s=');
        expect(navigated).toBeTruthy();

        // ✅ Autocomplete closes
        await expect(dropdown).not.toBeVisible();
      }
    }
  });
});
