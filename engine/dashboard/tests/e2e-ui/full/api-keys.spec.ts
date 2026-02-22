/**
 * E2E-UI Full Suite — API Keys Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests run against a REAL Flapjack server with seeded test data.
 *
 * NOTE: The /1/keys API is defined in the OpenAPI spec but not yet implemented
 * on the server (returns 404). Tests that require key creation/deletion are
 * skipped until the backend supports these operations.
 *
 * Covers:
 * - Page loads and shows empty state
 * - Create key dialog opens and shows all form sections
 * - Dialog permissions toggling works
 * - (Skipped) Create, delete, copy, scope tests — need backend /1/keys support
 */
import { test, expect } from '../../fixtures/auth.fixture';

test.describe('API Keys Page', () => {

  test.beforeEach(async ({ page }) => {
    await page.goto('/keys');
    await expect(
      page.getByRole('heading', { name: 'API Keys', exact: true })
    ).toBeVisible({ timeout: 10000 });
  });

  test('API keys page loads and shows heading and create button', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'API Keys', exact: true })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Create Key', exact: true })).toBeVisible();
  });

  test('create key dialog shows all form sections', async ({ page }) => {
    await page.getByRole('button', { name: 'Create Key', exact: true }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await expect(dialog.getByText('Description')).toBeVisible();
    await expect(dialog.getByText('Permissions').first()).toBeVisible();
    await expect(dialog.getByText('Search').first()).toBeVisible();
    await expect(dialog.getByText('Browse').first()).toBeVisible();
    await expect(dialog.getByText('Add Object').first()).toBeVisible();
    await expect(dialog.getByText('Delete Object').first()).toBeVisible();
    await expect(dialog.getByText('Delete Index').first()).toBeVisible();
    await expect(dialog.getByText('Settings').first()).toBeVisible();
    await expect(dialog.getByText('List Indexes').first()).toBeVisible();
    await expect(dialog.getByText('Analytics').first()).toBeVisible();

    await expect(dialog.getByText('Index Scope')).toBeVisible();
    await expect(dialog.getByText('Max Hits Per Query')).toBeVisible();
    await expect(dialog.getByText('Max Queries Per IP Per Hour')).toBeVisible();

    await expect(dialog.getByRole('button', { name: /cancel/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /create key/i })).toBeVisible();

    await dialog.getByRole('button', { name: /cancel/i }).click();
  });

  test('toggling permissions updates selection badges', async ({ page }) => {
    await page.getByRole('button', { name: 'Create Key', exact: true }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Default permission "search" should be shown in the selected permissions badges
    const permBadges = dialog.getByTestId('selected-permissions');
    await expect(permBadges.getByText('search').first()).toBeVisible();

    // Toggle "Add Object" on — badge should appear
    await dialog.getByRole('button', { name: /Add Object/i }).click();
    await expect(permBadges.getByText('addObject').first()).toBeVisible();

    // Toggle "Search" off — "addObject" should remain
    await dialog.getByRole('button', { name: /^Search/ }).click();
    await expect(permBadges.getByText('addObject').first()).toBeVisible();

    await dialog.getByRole('button', { name: /cancel/i }).click();
  });

  // ---------- Tests below require /1/keys backend support (not yet implemented) ----------

  test.skip('create a new API key and verify it appears in the list', async ({ page }) => {
    // Requires POST /1/keys (returns 404 on current server)
  });

  test.skip('create then delete an API key', async ({ page }) => {
    // Requires POST /1/keys + DELETE /1/keys/:key
  });

  test.skip('key cards display permissions badges', async ({ page }) => {
    // Requires POST /1/keys to create a key first
  });

  test.skip('copy button is visible on key cards', async ({ page }) => {
    // Requires POST /1/keys to create a key first
  });

  test.skip('clicking copy button shows Copied feedback', async ({ page }) => {
    // Requires POST /1/keys to create a key first
  });

  test.skip('key with no index scope shows All Indexes badge', async ({ page }) => {
    // Requires POST /1/keys to create a key first
  });

  test.skip('create key with restricted index scope shows specific index badge', async ({ page }) => {
    // Requires POST /1/keys to create a key first
  });
});
