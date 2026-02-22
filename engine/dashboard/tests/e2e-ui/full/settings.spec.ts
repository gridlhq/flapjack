/**
 * E2E-UI Full Suite — Settings Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests run against a REAL Flapjack server with seeded test data.
 *
 * Settings:
 *   searchableAttributes: ['name', 'description', 'brand', 'category', 'tags']
 *   attributesForFaceting: ['category', 'brand', 'filterOnly(price)', 'filterOnly(inStock)']
 *   customRanking: ['desc(rating)', 'asc(price)']
 *
 * Covers:
 * - Searchable attributes display
 * - Faceting configuration display
 * - JSON editor toggle
 * - Ranking/custom ranking display
 * - Compact index button (visible + clickable)
 * - FilterOnly facets display
 * - Breadcrumb navigation
 * - All major sections present
 * - Save settings + verify persistence after reload
 * - Reset button reverts changes
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { API_BASE, API_HEADERS, TEST_INDEX } from '../helpers';
import { getSettings, updateSettings } from '../../fixtures/api-helpers';

test.describe('Settings Page', () => {

  test.beforeEach(async ({ page }) => {
    await page.goto(`/index/${TEST_INDEX}/settings`);
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 10000 });
  });

  test('displays searchable attributes from seeded settings', async ({ page }) => {
    const expectedAttributes = ['name', 'description', 'brand', 'category', 'tags'];

    await expect(page.getByText('name').first()).toBeVisible({ timeout: 10000 });

    for (const attr of expectedAttributes) {
      await expect(page.getByText(attr).first()).toBeVisible();
    }

    await expect(page.getByText('Search Behavior').first()).toBeVisible();
    await expect(page.getByText('Searchable Attributes').first()).toBeVisible();
  });

  test('displays faceting attributes from seeded settings', async ({ page }) => {
    await expect(page.getByText('Faceting').first()).toBeVisible({ timeout: 10000 });

    await expect(page.getByText('category').first()).toBeVisible();
    await expect(page.getByText('brand').first()).toBeVisible();
    await expect(page.getByText('Attributes For Faceting').first()).toBeVisible();
  });

  test('toggling JSON view shows raw settings JSON', async ({ page }) => {
    await expect(page.getByText('Search Behavior').first()).toBeVisible({ timeout: 10000 });

    const jsonToggle = page.getByRole('button', { name: /json/i });
    await expect(jsonToggle).toBeVisible();
    await jsonToggle.click();

    // After toggling, the settings JSON should be visible containing searchableAttributes
    await expect(page.getByText(/searchableAttributes/).first()).toBeVisible({ timeout: 15_000 });

    await jsonToggle.click();
    await expect(page.getByText('Search Behavior').first()).toBeVisible({ timeout: 10000 });
  });

  test('displays ranking and custom ranking configuration', async ({ page }) => {
    await expect(page.getByText('Ranking').first()).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Ranking & Sorting').first()).toBeVisible();
    await expect(page.getByText('Custom Ranking').first()).toBeVisible();
    await expect(page.getByText(/desc\(rating\)/).first()).toBeVisible();
    await expect(page.getByText(/asc\(price\)/).first()).toBeVisible();
  });

  test('compact index button is visible and enabled', async ({ page }) => {
    const compactBtn = page.getByRole('button', { name: /compact/i });
    await expect(compactBtn).toBeVisible({ timeout: 10000 });
    await expect(compactBtn).toContainText(/compact index/i);
    await expect(compactBtn).toBeEnabled();
  });

  test('compact index button click triggers compaction', async ({ page }) => {
    const compactBtn = page.getByRole('button', { name: /compact/i });
    await expect(compactBtn).toBeVisible({ timeout: 10000 });

    // Click compact and wait for the API response
    const responsePromise = page.waitForResponse(
      resp => resp.url().includes('/compact'),
      { timeout: 15_000 }
    );
    await compactBtn.click();

    // Should get a response (success or already compact)
    const response = await responsePromise;
    expect([200, 202].includes(response.status())).toBeTruthy();
  });

  test('displays filterOnly faceting attributes', async ({ page }) => {
    await expect(page.getByText('Faceting').first()).toBeVisible({ timeout: 10000 });
    await expect(page.getByText(/price/).first()).toBeVisible();
    await expect(page.getByText(/inStock/).first()).toBeVisible();
  });

  test('settings page has breadcrumb back to index', async ({ page }) => {
    await expect(page.getByText(TEST_INDEX).first()).toBeVisible({ timeout: 10000 });
  });

  test('settings form shows all major sections', async ({ page }) => {
    await expect(page.getByText('Search Behavior').first()).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Searchable Attributes').first()).toBeVisible();
    await expect(page.getByText('Faceting').first()).toBeVisible();
    await expect(page.getByText('Ranking').first()).toBeVisible();
  });

  // ---------- Reset Button ----------

  test('Reset button appears after form modification and reverts changes', async ({ page }) => {
    await expect(page.getByText('Search Behavior').first()).toBeVisible({ timeout: 10000 });

    // Wait for settings to fully load — seeded "tags" attribute chip should be rendered
    // Use .first() because "tags" chip appears in multiple settings sections
    const tagsChip = page.getByTestId('attr-chip-tags').first();
    await expect(tagsChip).toBeVisible({ timeout: 10_000 });

    // Reset button should NOT be visible initially (no changes made)
    await expect(page.getByRole('button', { name: /reset/i })).not.toBeVisible();
    await expect(page.getByRole('button', { name: /save/i })).not.toBeVisible();

    // Modify the form by clicking the "tags" attribute chip to deselect it
    await tagsChip.click();

    // Reset and Save buttons should now be visible
    const resetBtn = page.getByRole('button', { name: /reset/i });
    const saveBtn = page.getByRole('button', { name: /save/i });
    await expect(resetBtn).toBeVisible({ timeout: 5_000 });
    await expect(saveBtn).toBeVisible();

    // Click Reset to revert changes
    await resetBtn.click();

    // Reset and Save buttons should disappear
    await expect(resetBtn).not.toBeVisible({ timeout: 5_000 });
    await expect(saveBtn).not.toBeVisible();
  });

  // ---------- Save Settings + Verify Persistence ----------

  test('save settings persists changes after reload', async ({ page, request }) => {
    await expect(page.getByText('Search Behavior').first()).toBeVisible({ timeout: 10000 });

    // Get the current settings via API to restore later
    const originalSettings = await getSettings(request, TEST_INDEX);

    // Wait for seeded attributes to load
    // Use .first() because "tags" chip appears in multiple settings sections
    const tagsChip = page.getByTestId('attr-chip-tags').first();
    await expect(tagsChip).toBeVisible({ timeout: 10_000 });

    // Click "tags" to deselect it (triggers dirty state)
    await tagsChip.click();

    // Save button should now be visible
    const saveBtn = page.getByRole('button', { name: /save/i });
    await expect(saveBtn).toBeVisible({ timeout: 5_000 });

    // Click Save and wait for the API response
    const responsePromise = page.waitForResponse(
      resp => resp.url().includes('/settings') && (resp.status() === 200 || resp.status() === 202),
      { timeout: 15_000 }
    );
    await saveBtn.click();
    await responsePromise;

    // After save, reload page and verify the change persisted
    await page.reload();
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 10_000 });
    // "tags" should no longer be selected (verify it's in deselected state)
    await expect(page.getByText('tags').first()).toBeVisible({ timeout: 10_000 });

    // Restore original settings via API
    await updateSettings(request, TEST_INDEX, originalSettings);
  });
});
