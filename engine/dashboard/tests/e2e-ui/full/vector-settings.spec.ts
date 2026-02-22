/**
 * E2E-UI Full Suite — Vector Search Settings (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests run against a REAL Flapjack server with seeded test data.
 *
 * Covers:
 * - Search mode section display and mode switching
 * - Embedder configuration via Add Embedder dialog
 * - Embedder deletion via confirm dialog
 * - Settings persistence after save + reload
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { TEST_INDEX } from '../helpers';
import {
  configureEmbedder,
  clearEmbedders,
  getSettings,
  updateSettings,
} from '../../fixtures/api-helpers';

test.describe('Vector Search Settings', () => {
  // Tests modify shared index settings — must run serially (not in parallel)
  test.describe.configure({ mode: 'serial' });

  let originalSettings: Record<string, unknown>;

  test.beforeEach(async ({ request, page }) => {
    // Save original settings for cleanup
    originalSettings = await getSettings(request, TEST_INDEX);

    // Seed a userProvided embedder for tests that need existing embedders
    await configureEmbedder(request, TEST_INDEX, 'default', {
      source: 'userProvided',
      dimensions: 384,
    });

    await page.goto(`/index/${TEST_INDEX}/settings`);
    await expect(
      page.getByRole('heading', { name: /settings/i }),
    ).toBeVisible({ timeout: 10_000 });
  });

  test.afterEach(async ({ request }) => {
    // Restore original settings + explicitly clear vector search fields.
    // getSettings doesn't return embedders/mode when at defaults, so a plain
    // PUT roundtrip won't clear them — we must include them explicitly.
    await updateSettings(request, TEST_INDEX, {
      ...originalSettings,
      embedders: {},
      mode: 'keywordSearch',
    });
  });

  // ---- Load-and-verify (10.21 vector-settings-1) ----

  test('displays search mode and embedders sections with seeded data', async ({
    page,
  }) => {
    // Search Mode section
    await expect(page.getByText('Search Mode').first()).toBeVisible({
      timeout: 10_000,
    });

    // Embedders section
    await expect(page.getByText('Embedders').first()).toBeVisible();
    await expect(
      page.getByText('Configure embedding models for vector search'),
    ).toBeVisible();

    // Seeded embedder card
    await expect(page.getByTestId('embedder-card-default')).toBeVisible();
    await expect(
      page.getByTestId('embedder-card-default').getByText('userProvided'),
    ).toBeVisible();
    await expect(
      page.getByTestId('embedder-card-default').getByText('384'),
    ).toBeVisible();
  });

  // ---- Set search mode (10.21 vector-settings-2) ----

  test('set search mode to Neural Search and verify persistence', async ({
    page,
  }) => {
    await expect(page.getByTestId('search-mode-select')).toBeVisible({
      timeout: 10_000,
    });

    // Select Neural Search
    await page.getByTestId('search-mode-select').selectOption('neuralSearch');

    // Save button should appear
    const saveBtn = page.getByRole('button', { name: /save/i });
    await expect(saveBtn).toBeVisible({ timeout: 5_000 });

    // Click Save and wait for response
    const responsePromise = page.waitForResponse(
      (resp) =>
        resp.url().includes('/settings') &&
        (resp.status() === 200 || resp.status() === 202),
      { timeout: 15_000 },
    );
    await saveBtn.click();
    await responsePromise;

    // Reload and verify persistence
    await page.reload();
    await expect(
      page.getByRole('heading', { name: /settings/i }),
    ).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId('search-mode-select')).toHaveValue(
      'neuralSearch',
      { timeout: 10_000 },
    );
  });

  // ---- Add embedder (10.21 vector-settings-3) ----

  test('add userProvided embedder via dialog', async ({ page }) => {
    await expect(page.getByTestId('add-embedder-btn')).toBeVisible({
      timeout: 10_000,
    });

    // Click Add Embedder
    await page.getByTestId('add-embedder-btn').click();

    // Dialog should open
    await expect(page.getByTestId('embedder-dialog')).toBeVisible({
      timeout: 5_000,
    });

    // Fill form
    await page.getByTestId('embedder-name-input').fill('test-emb');
    await page.getByTestId('embedder-source-select').selectOption('userProvided');
    await page.getByTestId('embedder-dimensions-input').fill('384');

    // Save in dialog
    await page.getByTestId('embedder-save-btn').click();

    // Dialog should close, new card should appear
    await expect(page.getByTestId('embedder-dialog')).not.toBeVisible({
      timeout: 5_000,
    });
    await expect(page.getByTestId('embedder-card-test-emb')).toBeVisible();

    // Save settings
    const saveBtn = page.getByRole('button', { name: /save/i });
    await expect(saveBtn).toBeVisible({ timeout: 5_000 });
    const responsePromise = page.waitForResponse(
      (resp) =>
        resp.url().includes('/settings') &&
        (resp.status() === 200 || resp.status() === 202),
      { timeout: 15_000 },
    );
    await saveBtn.click();
    await responsePromise;

    // Reload and verify persistence
    await page.reload();
    await expect(
      page.getByRole('heading', { name: /settings/i }),
    ).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId('embedder-card-test-emb')).toBeVisible({
      timeout: 10_000,
    });
  });

  // ---- Delete embedder (10.21 vector-settings-5) ----

  test('delete an embedder via confirm dialog', async ({ page }) => {
    // Verify seeded embedder exists
    await expect(page.getByTestId('embedder-card-default')).toBeVisible({
      timeout: 10_000,
    });

    // Click delete button
    await page.getByTestId('embedder-delete-default').click();

    // Confirm dialog should appear
    await expect(
      page.getByRole('heading', { name: /delete embedder/i }),
    ).toBeVisible({ timeout: 5_000 });
    await page.getByRole('button', { name: 'Confirm' }).click();

    // Card should disappear
    await expect(
      page.getByTestId('embedder-card-default'),
    ).not.toBeVisible({ timeout: 5_000 });

    // Save settings
    const saveBtn = page.getByRole('button', { name: /save/i });
    await expect(saveBtn).toBeVisible({ timeout: 5_000 });
    const responsePromise = page.waitForResponse(
      (resp) =>
        resp.url().includes('/settings') &&
        (resp.status() === 200 || resp.status() === 202),
      { timeout: 15_000 },
    );
    await saveBtn.click();
    await responsePromise;

    // Reload and verify persistence
    await page.reload();
    await expect(
      page.getByRole('heading', { name: /settings/i }),
    ).toBeVisible({ timeout: 10_000 });
    // Should show "No embedders configured" in the embedder panel
    // (scoped to avoid matching the SearchModeSection warning badge)
    await expect(
      page.getByTestId('embedder-panel').getByText('No embedders configured'),
    ).toBeVisible({ timeout: 10_000 });
  });

  // ---- Persistence (10.21 vector-settings-6) ----

  test('embedder settings persist after save and navigation', async ({
    page,
  }) => {
    await expect(page.getByTestId('add-embedder-btn')).toBeVisible({
      timeout: 10_000,
    });

    // Add a new embedder
    await page.getByTestId('add-embedder-btn').click();
    await expect(page.getByTestId('embedder-dialog')).toBeVisible({
      timeout: 5_000,
    });
    await page.getByTestId('embedder-name-input').fill('persist-test');
    await page.getByTestId('embedder-source-select').selectOption('userProvided');
    await page.getByTestId('embedder-dimensions-input').fill('256');
    await page.getByTestId('embedder-save-btn').click();
    await expect(page.getByTestId('embedder-dialog')).not.toBeVisible({
      timeout: 5_000,
    });

    // Save settings
    const saveBtn = page.getByRole('button', { name: /save/i });
    await expect(saveBtn).toBeVisible({ timeout: 5_000 });
    const responsePromise = page.waitForResponse(
      (resp) =>
        resp.url().includes('/settings') &&
        (resp.status() === 200 || resp.status() === 202),
      { timeout: 15_000 },
    );
    await saveBtn.click();
    await responsePromise;

    // Navigate away to search page
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i)),
    ).toBeVisible({ timeout: 15_000 });

    // Navigate back to settings
    await page.goto(`/index/${TEST_INDEX}/settings`);
    await expect(
      page.getByRole('heading', { name: /settings/i }),
    ).toBeVisible({ timeout: 10_000 });

    // Verify both embedders still present
    await expect(page.getByTestId('embedder-card-default')).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId('embedder-card-persist-test')).toBeVisible();
  });
});
