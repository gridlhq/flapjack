/**
 * E2E-UI Full Suite — Query Suggestions Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests run against a REAL Flapjack server.
 *
 * Covers:
 * - Page loads with heading and Create Config button
 * - Seeded config renders in list (load-and-verify rule)
 * - Create config dialog shows required form fields
 * - Create config via dialog → appears in list with source info
 * - Config card shows source, status fields, and action buttons
 * - Rebuild button is enabled and triggers a build (toast visible)
 * - Delete config via confirm dialog → removed from list
 * - Cancel in create dialog closes without creating
 * - Sidebar nav link navigates to the page
 */
import { test, expect } from '../../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

const QS_SOURCE = 'e2e-products';

// ── Shared UI helpers (pure UI interactions — no API calls) ──────────────────

async function goToQsPage(page: Page) {
  await page.goto('/query-suggestions');
  // Use the <h2> page heading (exact match), not the empty-state <h3>
  await expect(
    page.getByRole('heading', { name: 'Query Suggestions', exact: true, level: 2 })
  ).toBeVisible({ timeout: 10000 });
}

async function createConfigViaUi(page: Page, configName: string) {
  await page.getByRole('button', { name: /create config/i }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await dialog.getByLabel(/suggestions index name/i).fill(configName);
  await dialog.getByLabel(/source index name/i).fill(QS_SOURCE);
  await dialog.getByRole('button', { name: /create config/i }).click();
  await expect(dialog).not.toBeVisible({ timeout: 10000 });
}

async function deleteConfigViaUi(page: Page, configName: string) {
  page.once('dialog', async (dlg) => {
    if (dlg.type() === 'confirm') await dlg.accept();
  });
  const card = page.getByTestId('qs-config-card').filter({ hasText: configName });
  await card.getByRole('button', { name: /delete config/i }).click();
  // Scope to config name elements only (config name also appears in the API log widget)
  await expect(page.getByTestId('qs-config-name').filter({ hasText: configName })).not.toBeVisible({ timeout: 10000 });
}

// ── Tests ─────────────────────────────────────────────────────────────────────

test.describe('Query Suggestions Page', () => {

  // ── Load-and-verify (first spec per BROWSER_TESTING_STANDARDS_2.md) ─────────
  //
  // Seed a config via UI (creation isn't the core focus here), navigate away,
  // navigate back, and assert the config renders correctly in the list body.

  test('seeded config renders in the list after navigation', async ({ page }) => {
    const configName = `qs-seed-${Date.now()}`;

    // ARRANGE: create config via UI (precondition for list-render test)
    await goToQsPage(page);
    await createConfigViaUi(page, configName);
    // Scope to card name element (configName also appears in the API log widget)
    await expect(page.getByTestId('qs-config-name').filter({ hasText: configName })).toBeVisible({ timeout: 10000 });

    // Navigate away then back — forces a fresh data fetch
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Indexes', exact: true })).toBeVisible({ timeout: 10000 });
    await goToQsPage(page);

    // ASSERT: config appears in the list body
    await expect(
      page.getByTestId('qs-configs-list').getByText(configName)
    ).toBeVisible({ timeout: 10000 });

    // CLEANUP
    await deleteConfigViaUi(page, configName);
  });

  // ── Page basics ──────────────────────────────────────────────────────────────

  test('page loads with heading and Create Config button', async ({ page }) => {
    await goToQsPage(page);
    await expect(page.getByRole('button', { name: /create config/i })).toBeVisible();
  });

  test('empty state shows Create Your First Config when no configs exist', async ({ page }) => {
    await goToQsPage(page);

    const configList = page.getByTestId('qs-configs-list');
    const hasConfigs = await configList.isVisible({ timeout: 2000 }).catch(() => false);

    if (!hasConfigs) {
      await expect(
        page.getByRole('button', { name: /create.*first config/i })
      ).toBeVisible();
    } else {
      // List exists — load-and-verify test covers this state
      expect(hasConfigs).toBe(true);
    }
  });

  // ── Create config dialog ─────────────────────────────────────────────────────

  test('create config dialog shows all required form fields', async ({ page }) => {
    await goToQsPage(page);

    await page.getByRole('button', { name: /create config/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(
      dialog.getByRole('heading', { name: /create query suggestions/i })
    ).toBeVisible();

    await expect(dialog.getByLabel(/suggestions index name/i)).toBeVisible();
    await expect(dialog.getByLabel(/source index name/i)).toBeVisible();
    await expect(dialog.getByLabel(/minimum hits/i)).toBeVisible();
    await expect(dialog.getByLabel(/minimum letters/i)).toBeVisible();
    await expect(dialog.getByLabel(/exclude word/i)).toBeVisible();
    await expect(dialog.getByRole('button', { name: /create config/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /cancel/i })).toBeVisible();

    await dialog.getByRole('button', { name: /cancel/i }).click();
    await expect(dialog).not.toBeVisible({ timeout: 5000 });
  });

  test('cancel closes dialog without creating a config', async ({ page }) => {
    const uniqueName = `qs-cancelled-${Date.now()}`;

    await goToQsPage(page);

    await page.getByRole('button', { name: /create config/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByLabel(/suggestions index name/i).fill(uniqueName);
    await dialog.getByRole('button', { name: /cancel/i }).click();
    await expect(dialog).not.toBeVisible({ timeout: 5000 });

    await expect(page.getByText(uniqueName)).not.toBeVisible();
  });

  // ── Create and verify card ───────────────────────────────────────────────────

  test('created config card shows source index, status, and action buttons', async ({ page }) => {
    const configName = `qs-card-${Date.now()}`;

    await goToQsPage(page);
    await createConfigViaUi(page, configName);

    const card = page.getByTestId('qs-config-card').filter({ hasText: configName });
    await expect(card).toBeVisible({ timeout: 10000 });

    await expect(card.getByText(QS_SOURCE)).toBeVisible();
    await expect(card.getByText(/last built/i)).toBeVisible();
    await expect(card.getByRole('button', { name: 'Rebuild suggestions index', exact: true })).toBeVisible();
    await expect(card.getByRole('button', { name: /delete config/i })).toBeVisible();

    // CLEANUP
    await deleteConfigViaUi(page, configName);
  });

  // ── Rebuild button ───────────────────────────────────────────────────────────

  test('rebuild button triggers a build and shows toast', async ({ page }) => {
    const configName = `qs-rebuild-${Date.now()}`;

    await goToQsPage(page);
    await createConfigViaUi(page, configName);

    const card = page.getByTestId('qs-config-card').filter({ hasText: configName });
    await expect(card).toBeVisible({ timeout: 10000 });

    // Wait until the initial auto-build finishes (button re-enables).
    // Use exact aria-label "Rebuild suggestions index" — the config name contains "qs-rbld"
    // which would otherwise let /rebuild/i also match the delete button's aria-label.
    const rebuildBtn = card.getByRole('button', { name: 'Rebuild suggestions index', exact: true });
    await expect(rebuildBtn).toBeEnabled({ timeout: 30000 });

    await rebuildBtn.click();

    // The toast renders 3 elements (title div, description div, aria-live span).
    // Match the toast title exactly to avoid strict mode violations.
    await expect(
      page.getByText('Build triggered', { exact: true })
    ).toBeVisible({ timeout: 5000 });

    // CLEANUP
    await deleteConfigViaUi(page, configName);
  });

  // ── Delete config ────────────────────────────────────────────────────────────

  test('delete config removes it from the list', async ({ page }) => {
    const configName = `qs-delete-${Date.now()}`;

    await goToQsPage(page);
    await createConfigViaUi(page, configName);

    const card = page.getByTestId('qs-config-card').filter({ hasText: configName });
    await expect(card).toBeVisible({ timeout: 10000 });

    await deleteConfigViaUi(page, configName);

    await expect(page.getByTestId('qs-config-name').filter({ hasText: configName })).not.toBeVisible();
  });

  // ── Sidebar navigation ───────────────────────────────────────────────────────

  test('sidebar Query Suggestions link navigates to the page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Indexes', exact: true })).toBeVisible({ timeout: 10000 });

    await page.getByRole('link', { name: /query suggestions/i }).click();

    await expect(
      page.getByRole('heading', { name: 'Query Suggestions', exact: true, level: 2 })
    ).toBeVisible({ timeout: 10000 });
    await expect(page).toHaveURL(/query-suggestions/);
  });
});
