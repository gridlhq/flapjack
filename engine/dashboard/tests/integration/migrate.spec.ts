import { test, expect } from '../fixtures/auth.fixture';
import {
  hasAlgoliaCredentials,
  seedAlgoliaIndex,
  deleteAlgoliaIndex,
  deleteFlapjackIndex,
  type AlgoliaTestContext,
} from '../fixtures/algolia.fixture';
import { EXPECTED_COUNTS } from '../fixtures/test-data';

// ---------------------------------------------------------------------------
// Skip the entire suite gracefully if Algolia credentials are not available.
// ---------------------------------------------------------------------------
const hasCredentials = hasAlgoliaCredentials();
const describeOrSkip = hasCredentials ? test.describe : test.describe.skip;

describeOrSkip('Algolia Migration (E2E)', () => {
  let ctx: AlgoliaTestContext;

  // Integration tests need more time (Algolia indexing + migration + verification)
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

  test('should migrate an Algolia index via the UI and verify results', async ({ page }) => {
    // ----------------------------------------------------------------
    // 1. Navigate to the Migrate page
    // ----------------------------------------------------------------
    await page.goto('/migrate');
    await expect(page.getByRole('heading', { name: /migrate from algolia/i })).toBeVisible();

    // ----------------------------------------------------------------
    // 2. Fill in Algolia credentials
    // ----------------------------------------------------------------
    await page.locator('#app-id').fill(ctx.appId);
    await page.locator('#api-key').fill(ctx.adminKey);

    // ----------------------------------------------------------------
    // 3. Fill in index names
    // ----------------------------------------------------------------
    await page.locator('#source-index').fill(ctx.indexName);
    // Leave target-index blank — defaults to source name

    // ----------------------------------------------------------------
    // 4. Enable "Overwrite if exists" for idempotent re-runs
    // ----------------------------------------------------------------
    const overwriteSwitch = page.locator('#overwrite');
    await overwriteSwitch.click();
    await expect(overwriteSwitch).toHaveAttribute('data-state', 'checked');

    // ----------------------------------------------------------------
    // 5. Verify the Migrate button shows the index name and is enabled
    // ----------------------------------------------------------------
    const migrateButton = page.getByRole('button', {
      name: new RegExp(`Migrate.*"${ctx.indexName}"`),
    });
    await expect(migrateButton).toBeEnabled();

    // ----------------------------------------------------------------
    // 6. Click Migrate — wait for the API response (not a sleep!)
    // ----------------------------------------------------------------
    const responsePromise = page.waitForResponse(
      (resp) =>
        resp.url().includes('/1/migrate-from-algolia') && resp.status() === 200,
    );
    await migrateButton.click();

    // Button text should switch to "Migrating..." (auto-retrying assertion)
    await expect(page.getByText(/migrating from algolia/i)).toBeVisible();

    await responsePromise;

    // ----------------------------------------------------------------
    // 7. Wait for the success card to appear
    // ----------------------------------------------------------------
    const successHeading = page.getByText('Migration complete');
    await expect(successHeading).toBeVisible({ timeout: 30_000 });

    // ----------------------------------------------------------------
    // 8. Verify imported counts in the success card
    // ----------------------------------------------------------------
    // Each stat is a <div class="rounded-md border p-3"> containing
    //   <div class="text-xl font-bold">{value}</div>
    //   <div class="text-xs text-muted-foreground">{label}</div>

    // Documents
    const docsStatLabel = page.getByText('Documents', { exact: true });
    const docsStat = docsStatLabel.locator('..');
    await expect(docsStat.locator('.text-xl')).toHaveText(
      String(EXPECTED_COUNTS.documents),
    );

    // Settings
    const settingsStatLabel = page.getByText('Settings', { exact: true });
    const settingsStat = settingsStatLabel.locator('..');
    await expect(settingsStat.locator('.text-xl')).toHaveText('Applied');

    // Synonyms
    const synonymsStatLabel = page.getByText('Synonyms', { exact: true });
    const synonymsStat = synonymsStatLabel.locator('..');
    await expect(synonymsStat.locator('.text-xl')).toHaveText(
      String(EXPECTED_COUNTS.synonyms),
    );

    // Rules
    const rulesStatLabel = page.getByText('Rules', { exact: true });
    const rulesStat = rulesStatLabel.locator('..');
    await expect(rulesStat.locator('.text-xl')).toHaveText(
      String(EXPECTED_COUNTS.rules),
    );

    // ----------------------------------------------------------------
    // 9. Click "Browse Index" to navigate to the migrated index
    // ----------------------------------------------------------------
    await page.getByRole('link', { name: 'Browse Index' }).click();
    await expect(page).toHaveURL(
      new RegExp(`/index/${encodeURIComponent(ctx.indexName)}`),
    );

    // ----------------------------------------------------------------
    // 10. Verify documents are searchable in the Flapjack UI
    // ----------------------------------------------------------------
    await expect(
      page.getByRole('heading', { name: ctx.indexName }),
    ).toBeVisible({ timeout: 10_000 });

    // Wait for results to load — poll for the results panel or document count
    await expect(
      page.locator('[data-testid="results-panel"]'),
    ).toBeVisible({ timeout: 15_000 });

    // Search for "laptop" and verify results appear
    const searchInput = page.getByPlaceholder(/search documents/i);
    await searchInput.fill('laptop');
    await searchInput.press('Enter');

    // Verify search returned results — p01 (MacBook) should be the first hit
    await expect(
      page.getByText('p01').first(),
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should show error state for invalid credentials', async ({ page }) => {
    await page.goto('/migrate');

    await page.locator('#app-id').fill('INVALID_APP_ID');
    await page.locator('#api-key').fill('invalid_key_0000000000');
    await page.locator('#source-index').fill('nonexistent-index');

    const migrateButton = page.getByRole('button', { name: /migrate/i });
    await migrateButton.click();

    // Wait for the error card to appear (auto-retrying, no sleep)
    await expect(
      page.getByText('Migration failed'),
    ).toBeVisible({ timeout: 15_000 });
  });
});
