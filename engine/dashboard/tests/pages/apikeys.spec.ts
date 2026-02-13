import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

const MOCK_KEYS = {
  keys: [
    {
      value: 'abc123def456',
      description: 'Frontend search key',
      acl: ['search', 'browse'],
      indexes: ['products'],
      createdAt: Math.floor(Date.now() / 1000) - 86400,
    },
    {
      value: 'xyz789ghi012',
      description: 'Admin key',
      acl: ['search', 'browse', 'addObject', 'deleteObject', 'settings', 'listIndexes'],
      createdAt: Math.floor(Date.now() / 1000) - 172800,
    },
    {
      value: 'jkl345mno678',
      description: 'Analytics only',
      acl: ['analytics'],
      indexes: ['analytics-index'],
      createdAt: Math.floor(Date.now() / 1000),
    },
  ],
};

const MOCK_INDICES = {
  results: [
    { uid: 'products', name: 'products', entries: 100, dataSize: 5000 },
    { uid: 'analytics-index', name: 'analytics-index', entries: 50, dataSize: 2000 },
  ],
};

/**
 * Register mock routes for the keys API.
 * DELETE route (more specific glob) is registered first, then the exact-pathname
 * route for GET/POST so the two don't collide.
 */
async function mockKeysApi(page: Page, response = MOCK_KEYS) {
  // DELETE /1/keys/{value} — register first (more specific path)
  await page.route('**/1/keys/*', (route) => {
    if (route.request().method() === 'DELETE') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: '{}',
      });
    }
    return route.fallback();
  });

  // GET /POST on exactly /1/keys (no trailing segments)
  await page.route(
    (url) => url.pathname === '/1/keys',
    (route) => {
      if (route.request().method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(response),
        });
      }
      if (route.request().method() === 'POST') {
        const body = route.request().postDataJSON();
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({
            value: 'new-key-value-999',
            ...body,
            createdAt: Date.now(),
          }),
        });
      }
      return route.fallback();
    },
  );
}

async function mockIndicesApi(page: Page, response = MOCK_INDICES) {
  await page.route(
    (url) => url.pathname === '/1/indexes',
    (route) => {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(response),
      });
    },
  );
}

// ---------------------------------------------------------------------------
// Empty State
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Empty State', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page, { keys: [] });
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('shows "No API keys" empty state message', async ({ page }) => {
    await expect(page.getByText('No API keys')).toBeVisible();
    await expect(
      page.getByText(/create an api key to start making authenticated requests/i),
    ).toBeVisible();
  });

  test('shows "Create Your First Key" button', async ({ page }) => {
    await expect(
      page.getByRole('button', { name: /create your first key/i }),
    ).toBeVisible();
  });

  test('does not show filter bar', async ({ page }) => {
    await expect(page.getByText('No API keys')).toBeVisible();
    await expect(page.getByTestId('index-filter-bar')).not.toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// With Keys
// ---------------------------------------------------------------------------
test.describe('API Keys Page — With Keys', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('renders 3 key cards', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();
    await expect(page.getByTestId('key-card')).toHaveCount(3);
  });

  test('displays key descriptions', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();
    await expect(page.getByText('Frontend search key')).toBeVisible();
    await expect(page.getByText('Admin key')).toBeVisible();
    await expect(page.getByText('Analytics only')).toBeVisible();
  });

  test('displays permission badges for each key', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Frontend search key has search + browse
    const frontendCard = page.getByTestId('key-card').filter({ hasText: 'Frontend search key' });
    await expect(frontendCard.getByText('search', { exact: true })).toBeVisible();
    await expect(frontendCard.getByText('browse', { exact: true })).toBeVisible();

    // Admin key has 6 permissions
    const adminCard = page.getByTestId('key-card').filter({ hasText: 'Admin key' });
    await expect(adminCard.getByText('addObject', { exact: true })).toBeVisible();
    await expect(adminCard.getByText('deleteObject', { exact: true })).toBeVisible();
    await expect(adminCard.getByText('settings', { exact: true })).toBeVisible();
    await expect(adminCard.getByText('listIndexes', { exact: true })).toBeVisible();

    // Analytics only key has analytics
    const analyticsCard = page.getByTestId('key-card').filter({ hasText: 'Analytics only' });
    await expect(analyticsCard.getByText('analytics', { exact: true })).toBeVisible();
  });

  test('shows index restriction for scoped keys', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Frontend search key is restricted to "products"
    const frontendCard = page.getByTestId('key-card').filter({ hasText: 'Frontend search key' });
    const frontendScope = frontendCard.getByTestId('key-scope');
    await expect(frontendScope.getByText('products', { exact: true })).toBeVisible();

    // Analytics only key is restricted to "analytics-index"
    const analyticsCard = page.getByTestId('key-card').filter({ hasText: 'Analytics only' });
    const analyticsScope = analyticsCard.getByTestId('key-scope');
    await expect(analyticsScope.getByText('analytics-index', { exact: true })).toBeVisible();
  });

  test('admin key with no indexes restriction shows all-access indicator', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    const adminCard = page.getByTestId('key-card').filter({ hasText: 'Admin key' });
    const adminScope = adminCard.getByTestId('key-scope');
    await expect(adminScope.getByText('All Indices')).toBeVisible();
  });

  test('shows Copy button on each key card', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    const copyButtons = page.getByTestId('key-card').getByRole('button', { name: /copy/i });
    await expect(copyButtons).toHaveCount(3);
  });

  test('shows Delete button on each key card', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    const cards = page.getByTestId('key-card');
    for (let i = 0; i < 3; i++) {
      await expect(cards.nth(i).getByTestId('delete-key-btn')).toBeVisible();
    }
  });
});

// ---------------------------------------------------------------------------
// Filter Bar
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Filter Bar', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('shows filter bar with All button and index-specific buttons', async ({ page }) => {
    await expect(page.getByTestId('index-filter-bar')).toBeVisible();
    await expect(page.getByTestId('filter-all')).toBeVisible();
    await expect(page.getByTestId('filter-index-products')).toBeVisible();
    await expect(page.getByTestId('filter-index-analytics-index')).toBeVisible();
  });

  test('clicking an index filter shows only keys restricted to that index plus all-access keys', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();
    await expect(page.getByTestId('key-card')).toHaveCount(3);

    // Click "products" filter
    await page.getByTestId('filter-index-products').click();

    // Should show: Frontend search key (products) + Admin key (all-access, no indexes restriction)
    // Should hide: Analytics only (analytics-index only)
    await expect(page.getByTestId('key-card')).toHaveCount(2);
    await expect(page.getByText('Frontend search key')).toBeVisible();
    await expect(page.getByText('Admin key')).toBeVisible();
    await expect(page.getByText('Analytics only')).not.toBeVisible();
  });

  test('clicking All shows all keys again', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Filter first
    await page.getByTestId('filter-index-products').click();
    await expect(page.getByTestId('key-card')).toHaveCount(2);

    // Click All to reset
    await page.getByTestId('filter-all').click();
    await expect(page.getByTestId('key-card')).toHaveCount(3);
  });
});

// ---------------------------------------------------------------------------
// Create Key Dialog
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Create Key Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('clicking Create Key button opens dialog', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('Create API Key')).toBeVisible();
  });

  test('dialog shows description input', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByPlaceholder(/frontend search key/i)).toBeVisible();
  });

  test('dialog shows permission buttons', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // All 8 ACL options should be visible
    await expect(dialog.getByText('Search', { exact: true })).toBeVisible();
    await expect(dialog.getByText('Browse', { exact: true })).toBeVisible();
    await expect(dialog.getByText('Add Object', { exact: true })).toBeVisible();
    await expect(dialog.getByText('Delete Object', { exact: true })).toBeVisible();
    await expect(dialog.getByText('Delete Index', { exact: true })).toBeVisible();
    await expect(dialog.getByText('Settings', { exact: true })).toBeVisible();
    await expect(dialog.getByText('List Indexes', { exact: true })).toBeVisible();
    await expect(dialog.getByText('Analytics', { exact: true })).toBeVisible();
  });

  test('submitting with no permissions shows alert and keeps dialog open', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // By default "search" is pre-selected — deselect it so no permissions remain.
    await dialog.getByText('Search', { exact: true }).click();

    const createBtn = dialog.getByRole('button', { name: /create key/i });
    await expect(createBtn).toBeVisible();

    // Capture the alert message and auto-accept it
    let alertMessage = '';
    page.on('dialog', (d) => {
      alertMessage = d.message();
      return d.accept();
    });
    await createBtn.click();

    // Alert should have fired with a meaningful message
    expect(alertMessage).toContain('at least one permission');

    // Dialog should still be open because submission was blocked
    await expect(dialog).toBeVisible();
  });

  test('default permission "search" is pre-selected', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // "search" is pre-selected by default — verify the badge is visible
    await expect(dialog.getByText('search', { exact: true })).toBeVisible();
  });

  test('clicking Cancel closes dialog without creating', async ({ page }) => {
    // Track whether any POST to /1/keys happens
    let postFired = false;
    await page.route('**/1/keys', (route) => {
      if (route.request().method() === 'POST') {
        postFired = true;
      }
      return route.fallback();
    });

    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await dialog.getByRole('button', { name: /cancel/i }).click();
    await expect(dialog).not.toBeVisible();

    // Verify no POST request was sent
    expect(postFired).toBe(false);
  });

  test('creating a key sends correct POST body with acl and description', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Fill in description
    await dialog.getByPlaceholder(/frontend search key/i).fill('My new key');

    // "search" is pre-selected by default. Also select "browse".
    await dialog.getByText('Browse', { exact: true }).click();

    // Capture the POST request
    let lastBody: any = null;
    const responsePromise = page.waitForResponse(
      (response) => response.url().includes('/1/keys') && response.request().method() === 'POST',
    );

    // Intercept to capture body (re-route for POST capture)
    await page.route(
      (url) => url.pathname === '/1/keys',
      (route) => {
        if (route.request().method() === 'POST') {
          lastBody = route.request().postDataJSON();
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
              value: 'new-key-value-999',
              ...lastBody,
              createdAt: Math.floor(Date.now() / 1000),
            }),
          });
        }
        return route.fallback();
      },
    );

    // Click Create Key in the dialog footer
    await dialog.getByRole('button', { name: /create key/i }).click();
    await responsePromise;

    expect(lastBody).toBeTruthy();
    expect(lastBody.description).toBe('My new key');
    expect(lastBody.acl).toContain('search');
    expect(lastBody.acl).toContain('browse');
  });
});

// ---------------------------------------------------------------------------
// Create Key Dialog — Index Scope Selection
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Create Key Dialog (Index Scope)', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('shows Index Scope section with available indices', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const scopeSection = dialog.getByTestId('index-scope-section');
    await expect(scopeSection).toBeVisible();
    await expect(scopeSection.getByText('Index Scope')).toBeVisible();
    await expect(scopeSection.getByText(/access control/i)).toBeVisible();

    // Both mock indices should be shown as selectable buttons
    await expect(scopeSection.getByText('products', { exact: true })).toBeVisible();
    await expect(scopeSection.getByText('analytics-index', { exact: true })).toBeVisible();
  });

  test('clicking an index shows scope summary', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const scopeSection = dialog.getByTestId('index-scope-section');

    // No scope summary initially
    await expect(dialog.getByTestId('scope-summary')).not.toBeVisible();

    // Click "products" to select it
    await scopeSection.getByText('products', { exact: true }).click();

    // Scope summary should now appear
    const summary = dialog.getByTestId('scope-summary');
    await expect(summary).toBeVisible();
    await expect(summary.getByText('This key can access:')).toBeVisible();
    await expect(summary.getByText('products')).toBeVisible();
  });

  test('clicking a selected index deselects it', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const scopeSection = dialog.getByTestId('index-scope-section');

    // Select products
    await scopeSection.getByRole('button', { name: 'products' }).click();
    await expect(dialog.getByTestId('scope-summary')).toBeVisible();

    // Deselect products — use role selector to avoid matching the summary badge
    await scopeSection.getByRole('button', { name: 'products' }).click();
    await expect(dialog.getByTestId('scope-summary')).not.toBeVisible();
  });

  test('selecting multiple indices shows all in scope summary', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const scopeSection = dialog.getByTestId('index-scope-section');

    // Select both indices
    await scopeSection.getByText('products', { exact: true }).click();
    await scopeSection.getByText('analytics-index', { exact: true }).click();

    const summary = dialog.getByTestId('scope-summary');
    await expect(summary).toBeVisible();
    await expect(summary.getByText('products')).toBeVisible();
    await expect(summary.getByText('analytics-index')).toBeVisible();
  });

  test('creating a key with index scope sends indexes in POST body', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Select an index scope
    const scopeSection = dialog.getByTestId('index-scope-section');
    await scopeSection.getByText('products', { exact: true }).click();
    await expect(dialog.getByTestId('scope-summary')).toBeVisible();

    // Capture the POST request
    let lastBody: any = null;
    const responsePromise = page.waitForResponse(
      (response) => response.url().includes('/1/keys') && response.request().method() === 'POST',
    );

    await page.route(
      (url) => url.pathname === '/1/keys',
      (route) => {
        if (route.request().method() === 'POST') {
          lastBody = route.request().postDataJSON();
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({
              value: 'new-key-value-999',
              ...lastBody,
              createdAt: Math.floor(Date.now() / 1000),
            }),
          });
        }
        return route.fallback();
      },
    );

    await dialog.getByRole('button', { name: /create key/i }).click();
    await responsePromise;

    expect(lastBody).toBeTruthy();
    expect(lastBody.indexes).toContain('products');
    expect(lastBody.acl).toContain('search');
  });
});

// ---------------------------------------------------------------------------
// Create Key Dialog — No Indices
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Create Key Dialog (No Indices)', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page, { results: [] });
    await page.goto('/keys');
  });

  test('shows empty indices message in create dialog when no indices exist', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // The Index Scope section should show the empty state message
    const scopeSection = dialog.getByTestId('index-scope-section');
    await expect(scopeSection).toBeVisible();
    await expect(scopeSection.getByText(/no indices created yet/i)).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Delete Key
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Delete Key', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('clicking Delete on a key shows confirmation dialog', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Set up a dialog listener to capture the confirm prompt
    let confirmMessage = '';
    page.on('dialog', async (dialog) => {
      confirmMessage = dialog.message();
      await dialog.dismiss(); // dismiss = cancel
    });

    // Click the delete button on the first key card
    const firstCard = page.getByTestId('key-card').first();
    await firstCard.getByTestId('delete-key-btn').click();

    // Verify the native confirm was called with a meaningful message
    expect(confirmMessage).toContain('Frontend search key');
  });

  test('confirming delete sends DELETE request to correct URL', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Accept the confirm dialog
    page.on('dialog', async (dialog) => {
      await dialog.accept();
    });

    // Wait for the DELETE request to be made
    const deletePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/1/keys/abc123def456') &&
        response.request().method() === 'DELETE',
    );

    // Click delete on the first card (Frontend search key, value = abc123def456)
    const firstCard = page.getByTestId('key-card').first();
    await firstCard.getByTestId('delete-key-btn').click();

    const deleteResponse = await deletePromise;
    expect(deleteResponse.status()).toBe(200);
  });

  test('dismissing delete confirmation does not send DELETE', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    let deleteFired = false;
    await page.route('**/1/keys/*', (route) => {
      if (route.request().method() === 'DELETE') {
        deleteFired = true;
      }
      return route.fallback();
    });

    // Dismiss the confirm dialog (= cancel)
    page.on('dialog', async (dialog) => {
      await dialog.dismiss();
    });

    const firstCard = page.getByTestId('key-card').first();
    await firstCard.getByTestId('delete-key-btn').click();

    // After dialog dismiss, verify the keys list is still visible (UI settled)
    await expect(page.getByTestId('keys-list')).toBeVisible();
    expect(deleteFired).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Key Value Display
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Key Value Display', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('displays key values in code elements', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // The key values should be displayed in the card
    await expect(page.getByText('abc123def456')).toBeVisible();
    await expect(page.getByText('xyz789ghi012')).toBeVisible();
    await expect(page.getByText('jkl345mno678')).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Create Key Error Handling
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Create Key Error', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('handles server error when creating a key', async ({ page }) => {
    // Override POST to return 500
    await page.route(
      (url) => url.pathname === '/1/keys',
      (route) => {
        if (route.request().method() === 'POST') {
          return route.fulfill({
            status: 500,
            contentType: 'application/json',
            body: JSON.stringify({ error: 'Internal Server Error' }),
          });
        }
        return route.fallback();
      },
    );

    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Fill description and submit
    await dialog.getByPlaceholder(/frontend search key/i).fill('Test key');
    await dialog.getByRole('button', { name: /create key/i }).click();

    // Dialog should remain open since creation failed
    await expect(dialog).toBeVisible();
  });
});

// ─── Loading / Skeleton State ───────────────────────────────────────────────

test.describe('API Keys Page — Loading State', () => {
  test('shows skeleton loading state while keys are being fetched', async ({ page }) => {
    // Delay the API response to observe loading state
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
    });
    await page.route('**/1/keys', async (route) => {
      await new Promise((r) => setTimeout(r, 2000));
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ results: [] }),
      });
    });
    await page.goto('/keys');

    // Skeleton elements should be visible during load
    const skeletons = page.locator('.animate-pulse');
    await expect(skeletons.first()).toBeVisible();
  });
});

// ─── Creating... Pending State ──────────────────────────────────────────────

test.describe('API Keys Page — Creating Pending State', () => {
  test('shows "Creating..." text on button while create is pending', async ({ page }) => {
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
    });
    await page.route('**/1/keys', (route) => {
      if (route.request().method() === 'GET') {
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ results: [] }),
        });
      } else if (route.request().method() === 'POST') {
        // Delay POST to observe pending state
        setTimeout(() => {
          route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ key: 'new-key-123', description: 'Test key', actions: ['search'], expiresAt: null }),
          });
        }, 2000);
      } else {
        route.fallback();
      }
    });

    await page.goto('/keys');

    // Open create dialog
    await page.getByRole('button', { name: /create/i }).first().click();
    await expect(page.getByRole('dialog')).toBeVisible();

    // Fill in required fields (placeholder is "e.g., Frontend search key")
    await page.getByPlaceholder(/frontend search key/i).fill('Test Key');

    // Click Create Key — button should show "Creating..."
    await page.getByRole('button', { name: /^create key$/i }).click();

    // The button text should change to "Creating..." while pending
    await expect(page.getByText(/creating/i)).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// API Keys Page -- Clipboard Copy
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Clipboard Copy', () => {
  test('clicking Copy button shows "Copied" feedback', async ({ page, context }) => {
    // Grant clipboard permissions for the test
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);

    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');

    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Find the first Copy button
    const firstCard = page.getByTestId('key-card').first();
    const copyBtn = firstCard.getByRole('button', { name: /^copy$/i });
    await expect(copyBtn).toBeVisible();

    // Click Copy
    await copyBtn.click();

    // Button should now say "Copied"
    await expect(firstCard.getByText('Copied')).toBeVisible();

    // After 2s timeout it should revert to "Copy"
    await expect(firstCard.getByRole('button', { name: /^copy$/i })).toBeVisible({ timeout: 5000 });
  });
});

// ---------------------------------------------------------------------------
// API Keys Page -- Rate Limit Inputs
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Rate Limit Inputs', () => {
  test.beforeEach(async ({ page }) => {
    await mockKeysApi(page);
    await mockIndicesApi(page);
    await page.goto('/keys');
  });

  test('create dialog shows Max Hits Per Query input', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await expect(dialog.getByText('Max Hits Per Query')).toBeVisible();
    const input = dialog.locator('input[placeholder="Unlimited"]').first();
    await expect(input).toBeVisible();
    await input.fill('100');
    await expect(input).toHaveValue('100');
  });

  test('create dialog shows Max Queries Per IP Per Hour input', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    await expect(dialog.getByText('Max Queries Per IP Per Hour')).toBeVisible();
    const input = dialog.locator('input[placeholder="Unlimited"]').nth(1);
    await expect(input).toBeVisible();
    await input.fill('500');
    await expect(input).toHaveValue('500');
  });

  test('creating key with rate limits sends values in POST body', async ({ page }) => {
    let postBody: any = null;
    await page.route(
      (url) => url.pathname === '/1/keys',
      (route) => {
        if (route.request().method() === 'POST') {
          postBody = route.request().postDataJSON();
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ value: 'new-key', ...postBody, createdAt: Date.now() }),
          });
        }
        return route.fallback();
      },
    );

    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');

    // Fill rate limit fields
    const unlimitedInputs = dialog.locator('input[placeholder="Unlimited"]');
    await unlimitedInputs.first().fill('50');
    await unlimitedInputs.nth(1).fill('1000');

    // Click Create Key
    await dialog.getByRole('button', { name: /^create key$/i }).click();

    await expect.poll(() => postBody).not.toBeNull();
    expect(postBody.maxHitsPerQuery).toBe(50);
    expect(postBody.maxQueriesPerIPPerHour).toBe(1000);
  });
});

// ---------------------------------------------------------------------------
// API Keys Page -- Key Details Display
// ---------------------------------------------------------------------------
test.describe('API Keys Page — Key Details Display', () => {
  test('shows key detail fields (max hits, created date)', async ({ page }) => {
    const keysWithDetails = {
      keys: [
        {
          value: 'key-with-details',
          description: 'Detailed key',
          acl: ['search'],
          indexes: ['products'],
          maxHitsPerQuery: 200,
          maxQueriesPerIPPerHour: 5000,
          createdAt: 1704067200, // 2024-01-01
        },
      ],
    };
    await mockKeysApi(page, keysWithDetails);
    await mockIndicesApi(page);
    await page.goto('/keys');

    const card = page.getByTestId('key-card').first();
    await expect(card).toBeVisible();

    // Details grid should show max hits and max queries
    await expect(card.getByText('Max Hits/Query')).toBeVisible();
    await expect(card.getByText('200')).toBeVisible();
    await expect(card.getByText('Max Queries/IP/Hour')).toBeVisible();
    await expect(card.getByText('5,000')).toBeVisible();

    // Created date should be visible
    await expect(card.getByText('Created')).toBeVisible();
  });
});
