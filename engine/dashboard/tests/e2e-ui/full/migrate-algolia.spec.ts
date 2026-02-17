import { test, expect } from '../../fixtures/auth.fixture';
import {
  hasAlgoliaCredentials,
  seedAlgoliaIndex,
  deleteAlgoliaIndex,
  deleteFlapjackIndex,
  type AlgoliaTestContext,
} from '../../fixtures/algolia.fixture';
import { EXPECTED_COUNTS } from '../../fixtures/test-data';

/**
 * Algolia Migration — E2E-UI (real browser, real server, no mocks)
 *
 * Tests the full Algolia migration flow through the browser UI:
 * fill credentials → select index → click Migrate → verify success card → browse results.
 *
 * Requires Algolia credentials in .env.secret. Skips gracefully when unavailable.
 */

const hasCredentials = hasAlgoliaCredentials();
const describeOrSkip = hasCredentials ? test.describe : test.describe.skip;

describeOrSkip('Algolia Migration (real browser)', () => {
  let ctx: AlgoliaTestContext;

  test.describe.configure({ timeout: 120_000 });

  test.beforeAll(async () => {
    ctx = await seedAlgoliaIndex();
  });

  test.afterAll(async () => {
    await Promise.all([
      deleteAlgoliaIndex(ctx),
      deleteFlapjackIndex(ctx.indexName),
    ]);
  });

  test('migrate Algolia index via UI: fill form → migrate → verify success → browse', async ({ page }) => {
    // Navigate to Migrate page
    await page.goto('/migrate');
    await expect(page.getByRole('heading', { name: /migrate from algolia/i })).toBeVisible();

    // Fill in Algolia credentials
    await page.locator('#app-id').fill(ctx.appId);
    await page.locator('#api-key').fill(ctx.adminKey);
    await page.locator('#source-index').fill(ctx.indexName);

    // Enable overwrite
    const overwriteSwitch = page.locator('#overwrite');
    await overwriteSwitch.click();
    await expect(overwriteSwitch).toHaveAttribute('data-state', 'checked');

    // Verify Migrate button shows index name and is enabled
    const migrateButton = page.getByRole('button', {
      name: new RegExp(`Migrate.*"${ctx.indexName}"`),
    });
    await expect(migrateButton).toBeEnabled();

    // Click Migrate and wait for API response
    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/1/migrate-from-algolia') && resp.status() === 200,
    );
    await migrateButton.click();
    await expect(page.getByText(/migrating from algolia/i)).toBeVisible();
    await responsePromise;

    // Wait for success card
    await expect(page.getByText('Migration complete')).toBeVisible({ timeout: 30_000 });

    // Verify imported counts
    const docsStatLabel = page.getByText('Documents', { exact: true });
    await expect(docsStatLabel.locator('..').locator('.text-xl')).toHaveText(String(EXPECTED_COUNTS.documents));

    const settingsStatLabel = page.getByText('Settings', { exact: true });
    await expect(settingsStatLabel.locator('..').locator('.text-xl')).toHaveText('Applied');

    const synonymsStatLabel = page.getByText('Synonyms', { exact: true });
    await expect(synonymsStatLabel.locator('..').locator('.text-xl')).toHaveText(String(EXPECTED_COUNTS.synonyms));

    const rulesStatLabel = page.getByText('Rules', { exact: true });
    await expect(rulesStatLabel.locator('..').locator('.text-xl')).toHaveText(String(EXPECTED_COUNTS.rules));

    // Click "Browse Index" and verify navigation
    await page.getByRole('link', { name: 'Browse Index' }).click();
    await expect(page).toHaveURL(new RegExp(`/index/${encodeURIComponent(ctx.indexName)}`));

    // Verify documents are searchable
    await expect(page.getByRole('heading', { name: ctx.indexName })).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('[data-testid="results-panel"]')).toBeVisible({ timeout: 15_000 });

    const searchInput = page.getByPlaceholder(/search documents/i);
    await searchInput.fill('laptop');
    await searchInput.press('Enter');
    await expect(page.getByText('p01').first()).toBeVisible({ timeout: 10_000 });
  });

  test('invalid credentials show error state in UI', async ({ page }) => {
    await page.goto('/migrate');
    await page.locator('#app-id').fill('INVALID_APP_ID');
    await page.locator('#api-key').fill('invalid_key_0000000000');
    await page.locator('#source-index').fill('nonexistent-index');

    const migrateButton = page.getByRole('button', { name: /migrate/i });
    await migrateButton.click();

    await expect(page.getByText('Migration failed')).toBeVisible({ timeout: 15_000 });
  });
});
