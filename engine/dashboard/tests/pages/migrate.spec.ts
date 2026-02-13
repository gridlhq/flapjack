import { test, expect } from '../fixtures/auth.fixture';

test.describe('Migrate Page', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/migrate');
  });

  test('should display page heading and description', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /migrate from algolia/i })).toBeVisible();
    await expect(page.getByText(/import an index from algolia/i)).toBeVisible();
  });

  test('should display credentials card with inputs', async ({ page }) => {
    await expect(page.getByText('Algolia Credentials')).toBeVisible();
    await expect(page.locator('#app-id')).toBeVisible();
    await expect(page.locator('#api-key')).toBeVisible();
    await expect(page.getByText('Needs read access. Not stored anywhere.')).toBeVisible();
  });

  test('should display index card with inputs', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Index' })).toBeVisible();
    await expect(page.locator('#source-index')).toBeVisible();
    await expect(page.locator('#target-index')).toBeVisible();
    await expect(page.getByText('Defaults to the source index name if left blank.')).toBeVisible();
  });

  test('should toggle API key visibility', async ({ page }) => {
    const keyInput = page.locator('#api-key');

    // Starts as password field
    await expect(keyInput).toHaveAttribute('type', 'password');

    // Click the show/hide toggle (button inside the input wrapper)
    await keyInput.locator('..').getByRole('button').click();
    await expect(keyInput).toHaveAttribute('type', 'text');

    // Click again to hide
    await keyInput.locator('..').getByRole('button').click();
    await expect(keyInput).toHaveAttribute('type', 'password');
  });

  test('should disable migrate button when required fields are empty', async ({ page }) => {
    const migrateButton = page.getByRole('button', { name: /migrate/i });
    await expect(migrateButton).toBeDisabled();
  });

  test('should enable migrate button when required fields are filled', async ({ page }) => {
    await page.locator('#app-id').fill('test-app');
    await page.locator('#api-key').fill('test-key');
    await page.locator('#source-index').fill('my-index');

    const migrateButton = page.getByRole('button', { name: /migrate.*"my-index"/i });
    await expect(migrateButton).toBeEnabled();
  });

  test('should update button text with target index name', async ({ page }) => {
    await page.locator('#app-id').fill('test-app');
    await page.locator('#api-key').fill('test-key');
    await page.locator('#source-index').fill('source');

    // Button shows source name by default
    await expect(page.getByRole('button', { name: /migrate.*"source"/i })).toBeVisible();

    // Fill a different target name
    await page.locator('#target-index').fill('custom-target');
    await expect(page.getByRole('button', { name: /migrate.*"custom-target"/i })).toBeVisible();
  });

  test('should update target placeholder to match source input', async ({ page }) => {
    const targetInput = page.locator('#target-index');
    await expect(targetInput).toHaveAttribute('placeholder', 'Same as source');

    await page.locator('#source-index').fill('products');
    await expect(targetInput).toHaveAttribute('placeholder', 'products');
  });

  test('should display overwrite toggle', async ({ page }) => {
    const overwriteSwitch = page.locator('#overwrite');
    await expect(overwriteSwitch).toBeVisible();
    await expect(page.getByText('Overwrite if exists')).toBeVisible();
  });

  test('should display info section with migration details', async ({ page }) => {
    await expect(page.getByText(/what gets migrated/i)).toBeVisible();
    // "Credentials:" appears as bold label in the info section (distinct from the card title "Algolia Credentials")
    await expect(page.getByText('Credentials:', { exact: false })).toBeVisible();
    await expect(page.getByText(/large indexes/i)).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Migrate Page — Submission Flow
// ---------------------------------------------------------------------------

const MOCK_MIGRATION_SUCCESS = {
  status: 'ok',
  settings: true,
  synonyms: { imported: 12 },
  rules: { imported: 5 },
  objects: { imported: 2500 },
  taskID: 42,
};

test.describe('Migrate Page — Submission Flow', () => {
  test('sends POST to /1/migrate-from-algolia with correct body', async ({ page }) => {
    let capturedBody: any = null;
    await page.route('**/1/migrate-from-algolia', (route) => {
      capturedBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_MIGRATION_SUCCESS),
      });
    });
    // Mock indices so the query invalidation doesn't error
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('my-app');
    await page.locator('#api-key').fill('my-key');
    await page.locator('#source-index').fill('products');

    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/1/migrate-from-algolia') && resp.status() === 200,
    );
    await page.getByRole('button', { name: /migrate/i }).click();
    await responsePromise;

    expect(capturedBody).toBeTruthy();
    expect(capturedBody.appId).toBe('my-app');
    expect(capturedBody.apiKey).toBe('my-key');
    expect(capturedBody.sourceIndex).toBe('products');
  });

  test('sends targetIndex in body when specified', async ({ page }) => {
    let capturedBody: any = null;
    await page.route('**/1/migrate-from-algolia', (route) => {
      capturedBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_MIGRATION_SUCCESS),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('source-idx');
    await page.locator('#target-index').fill('target-idx');

    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/1/migrate-from-algolia') && resp.status() === 200,
    );
    await page.getByRole('button', { name: /migrate/i }).click();
    await responsePromise;

    expect(capturedBody.targetIndex).toBe('target-idx');
  });

  test('sends overwrite=true in body when toggle is on', async ({ page }) => {
    let capturedBody: any = null;
    await page.route('**/1/migrate-from-algolia', (route) => {
      capturedBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_MIGRATION_SUCCESS),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('idx');
    await page.locator('#overwrite').click();

    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/1/migrate-from-algolia') && resp.status() === 200,
    );
    await page.getByRole('button', { name: /migrate/i }).click();
    await responsePromise;

    expect(capturedBody.overwrite).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Migrate Page — Success State
// ---------------------------------------------------------------------------

test.describe('Migrate Page — Success State', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/1/migrate-from-algolia', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_MIGRATION_SUCCESS),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });
    await page.goto('/migrate');

    // Fill and submit
    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('products');

    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/1/migrate-from-algolia') && resp.status() === 200,
    );
    await page.getByRole('button', { name: /migrate/i }).click();
    await responsePromise;
  });

  test('shows "Migration complete" heading', async ({ page }) => {
    await expect(page.getByText('Migration complete')).toBeVisible();
  });

  test('shows index name in success message', async ({ page }) => {
    await expect(page.getByText(/products.*is ready|is ready/)).toBeVisible();
  });

  test('shows document count stat (2,500)', async ({ page }) => {
    // Scope to the success card to avoid matching page description text
    const successCard = page.locator('.border-green-500\\/50');
    await expect(successCard.getByText('2,500')).toBeVisible();
    await expect(successCard.getByText('Documents', { exact: true })).toBeVisible();
  });

  test('shows synonyms count stat (12)', async ({ page }) => {
    const successCard = page.locator('.border-green-500\\/50');
    await expect(successCard.getByText('12')).toBeVisible();
    await expect(successCard.getByText('Synonyms', { exact: true })).toBeVisible();
  });

  test('shows rules count stat (5)', async ({ page }) => {
    const successCard = page.locator('.border-green-500\\/50');
    await expect(successCard.getByText('5', { exact: true })).toBeVisible();
    await expect(successCard.getByText('Rules', { exact: true })).toBeVisible();
  });

  test('shows settings applied stat', async ({ page }) => {
    const successCard = page.locator('.border-green-500\\/50');
    await expect(successCard.getByText('Applied')).toBeVisible();
    await expect(successCard.getByText('Settings', { exact: true })).toBeVisible();
  });

  test('shows Browse Index link', async ({ page }) => {
    const browseLink = page.getByRole('link', { name: /browse index/i });
    await expect(browseLink).toBeVisible();
    await expect(browseLink).toHaveAttribute('href', '/index/products');
  });

  test('shows View Settings link', async ({ page }) => {
    const settingsLink = page.getByRole('link', { name: /view settings/i });
    await expect(settingsLink).toBeVisible();
    await expect(settingsLink).toHaveAttribute('href', '/index/products/settings');
  });
});

// ---------------------------------------------------------------------------
// Migrate Page — Loading State
// ---------------------------------------------------------------------------

test.describe('Migrate Page — Loading State', () => {
  test('shows loading spinner and disables inputs during migration', async ({ page }) => {
    // Use a route that never resolves to keep the pending state
    await page.route('**/1/migrate-from-algolia', () => {
      // Intentionally never fulfill — keeps mutation in pending state
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('idx');

    await page.getByRole('button', { name: /migrate/i }).click();

    // Button should show loading text
    await expect(page.getByText(/migrating from algolia/i)).toBeVisible();

    // Inputs should be disabled
    await expect(page.locator('#app-id')).toBeDisabled();
    await expect(page.locator('#api-key')).toBeDisabled();
    await expect(page.locator('#source-index')).toBeDisabled();
    await expect(page.locator('#target-index')).toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// Migrate Page — Error State
// ---------------------------------------------------------------------------

test.describe('Migrate Page — Error State (generic)', () => {
  test('shows "Migration failed" with error message on 500', async ({ page }) => {
    await page.route('**/1/migrate-from-algolia', (route) => {
      route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ message: 'Internal Server Error' }),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('idx');
    await page.getByRole('button', { name: /migrate/i }).click();

    await expect(page.getByText('Migration failed')).toBeVisible();
  });
});

test.describe('Migrate Page — Error State (409 conflict)', () => {
  test('shows overwrite hint on 409 conflict error', async ({ page }) => {
    await page.route('**/1/migrate-from-algolia', (route) => {
      route.fulfill({
        status: 409,
        contentType: 'application/json',
        body: JSON.stringify({}),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('idx');
    await page.getByRole('button', { name: /migrate/i }).click();

    // Error card should show the 409-specific message
    await expect(page.getByText('Migration failed')).toBeVisible();
    await expect(page.getByText('Target index already exists.')).toBeVisible();
  });
});

test.describe('Migrate Page — Error State (502 connection)', () => {
  test('shows Algolia connection error on 502', async ({ page }) => {
    await page.route('**/1/migrate-from-algolia', (route) => {
      route.fulfill({
        status: 502,
        contentType: 'application/json',
        body: JSON.stringify({}),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });

    await page.goto('/migrate');

    await page.locator('#app-id').fill('app1');
    await page.locator('#api-key').fill('key1');
    await page.locator('#source-index').fill('idx');
    await page.getByRole('button', { name: /migrate/i }).click();

    await expect(page.getByText(/could not connect to algolia/i)).toBeVisible();
  });
});
