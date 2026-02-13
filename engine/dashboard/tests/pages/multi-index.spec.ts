import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

/**
 * Multi-index integration tests — verifies the full workflow of
 * navigating between indices and viewing scoped API keys.
 */

const MOCK_INDICES = {
  items: [
    { uid: 'acme-products', name: 'acme-products', createdAt: '2024-01-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 500, dataSize: 25000, numberOfPendingTasks: 0 },
    { uid: 'globex-inventory', name: 'globex-inventory', createdAt: '2024-02-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 200, dataSize: 10000, numberOfPendingTasks: 0 },
    { uid: 'shared-catalog', name: 'shared-catalog', createdAt: '2024-03-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 1000, dataSize: 50000, numberOfPendingTasks: 0 },
  ],
  nbPages: 1,
};

const MOCK_KEYS = [
  {
    value: 'key-acme-search',
    description: 'Acme Search Key',
    acl: ['search'],
    indexes: ['acme-products'],
    createdAt: 1704067200,
  },
  {
    value: 'key-globex-full',
    description: 'Globex Full Access',
    acl: ['search', 'addObject', 'deleteObject'],
    indexes: ['globex-inventory'],
    createdAt: 1704067200,
  },
  {
    value: 'key-admin-global',
    description: 'Global Admin Key',
    acl: ['search', 'addObject', 'deleteObject', 'settings', 'deleteIndex'],
    indexes: [],
    createdAt: 1704067200,
  },
  {
    value: 'key-cross-index',
    description: 'Cross-Index Key',
    acl: ['search'],
    indexes: ['acme-products', 'globex-inventory'],
    createdAt: 1704067200,
  },
];

function mockApis(page: Page) {
  return Promise.all([
    page.route('**/1/indexes', (route) => {
      if (route.request().method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(MOCK_INDICES),
        });
      }
      return route.fallback();
    }),
    page.route('**/1/keys', (route) => {
      if (route.request().method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ keys: MOCK_KEYS }),
        });
      }
      return route.fallback();
    }),
  ]);
}

test.describe('Multi-Index — Sidebar Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
  });

  test('should show all indices in sidebar', async ({ page }) => {
    await page.goto('/');

    const sidebar = page.locator('aside');
    await expect(sidebar.getByTestId('sidebar-index-acme-products')).toBeVisible();
    await expect(sidebar.getByTestId('sidebar-index-globex-inventory')).toBeVisible();
    await expect(sidebar.getByTestId('sidebar-index-shared-catalog')).toBeVisible();
  });

  test('should navigate between indices from sidebar', async ({ page }) => {
    await page.goto('/');

    // Click Acme index
    await page.getByTestId('sidebar-index-acme-products').click();
    await expect(page).toHaveURL(/\/index\/acme-products/);

    // Click Globex index
    await page.getByTestId('sidebar-index-globex-inventory').click();
    await expect(page).toHaveURL(/\/index\/globex-inventory/);

    // Active state should switch
    await expect(page.getByTestId('sidebar-index-globex-inventory')).toHaveClass(/bg-primary/);
    await expect(page.getByTestId('sidebar-index-acme-products')).not.toHaveClass(/bg-primary/);
  });
});

test.describe('Multi-Index — Scoped Keys', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
    await page.goto('/keys');
  });

  test('should show scope for each key type', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Acme search key — scoped to acme-products index
    const acmeCard = page.getByTestId('key-card').filter({ hasText: 'Acme Search Key' });
    await expect(acmeCard.getByText('acme-products')).toBeVisible();

    // Global admin key — no restrictions
    const adminCard = page.getByTestId('key-card').filter({ hasText: 'Global Admin Key' });
    await expect(adminCard.getByText('All Indices')).toBeVisible();

    // Cross-index key — scoped to both
    const crossCard = page.getByTestId('key-card').filter({ hasText: 'Cross-Index Key' });
    await expect(crossCard.getByText('acme-products')).toBeVisible();
    await expect(crossCard.getByText('globex-inventory')).toBeVisible();
  });

  test('should filter to show only Acme-accessible keys', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Initially 4 keys
    await expect(page.getByTestId('key-card')).toHaveCount(4);

    // Filter by acme-products index
    await page.getByTestId('filter-index-acme-products').click();

    // Should show: Acme Search Key, Global Admin Key, Cross-Index Key (3 keys)
    await expect(page.getByTestId('key-card')).toHaveCount(3);
    await expect(page.getByText('Acme Search Key')).toBeVisible();
    await expect(page.getByText('Global Admin Key')).toBeVisible();
    await expect(page.getByText('Cross-Index Key')).toBeVisible();
    // Globex-only key should be hidden
    await expect(page.getByText('Globex Full Access')).not.toBeVisible();
  });

  test('should filter to show only Globex-accessible keys', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Filter by globex-inventory
    await page.getByTestId('filter-index-globex-inventory').click();

    // Should show: Globex Full Access, Global Admin Key, Cross-Index Key (3 keys)
    await expect(page.getByTestId('key-card')).toHaveCount(3);
    await expect(page.getByText('Globex Full Access')).toBeVisible();
    await expect(page.getByText('Global Admin Key')).toBeVisible();
    await expect(page.getByText('Cross-Index Key')).toBeVisible();
    // Acme-only key should be hidden
    await expect(page.getByText('Acme Search Key')).not.toBeVisible();
  });

  test('should filter to show only shared-catalog-accessible keys', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Filter by shared-catalog — only the global admin key has access
    await page.getByTestId('filter-index-shared-catalog').click();

    await expect(page.getByTestId('key-card')).toHaveCount(1);
    await expect(page.getByText('Global Admin Key')).toBeVisible();
  });
});

test.describe('Multi-Index — Create Key with Index Scope', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
    await page.goto('/keys');
  });

  test('creating key with index scope sends indexes in POST body', async ({ page }) => {
    let postBody: any = null;
    await page.route(
      (url) => url.pathname === '/1/keys',
      (route) => {
        if (route.request().method() === 'POST') {
          postBody = route.request().postDataJSON();
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ value: 'new-scoped-key', ...postBody, createdAt: Date.now() }),
          });
        }
        return route.fallback();
      },
    );

    // Open create dialog
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Select acme-products index in scope
    const scopeSection = dialog.getByTestId('index-scope-section');
    await expect(scopeSection).toBeVisible();
    await scopeSection.getByText('acme-products').click();

    // Scope summary should appear
    await expect(dialog.getByTestId('scope-summary')).toBeVisible();
    await expect(dialog.getByTestId('scope-summary').getByText('acme-products')).toBeVisible();

    // Create the key
    await dialog.getByRole('button', { name: /^create key$/i }).click();

    await expect.poll(() => postBody).not.toBeNull();
    expect(postBody.indexes).toEqual(['acme-products']);
  });
});

test.describe('Multi-Index — Filter Index Styling', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
    await page.goto('/keys');
  });

  test('filter-index button for index not in any key scope is visible', async ({ page }) => {
    // shared-catalog is an index but no key is scoped to it (only global admin has implicit access)
    await expect(page.getByTestId('index-filter-bar')).toBeVisible();
    const sharedBtn = page.getByTestId('filter-index-shared-catalog');
    await expect(sharedBtn).toBeVisible();
    await expect(sharedBtn).toHaveText('shared-catalog');
  });

  test('clicking filter-index for unscoped index shows only all-access keys', async ({ page }) => {
    await expect(page.getByTestId('keys-list')).toBeVisible();

    // Click shared-catalog filter — only the global admin key should be shown
    await page.getByTestId('filter-index-shared-catalog').click();
    await expect(page.getByTestId('key-card')).toHaveCount(1);
    await expect(page.getByText('Global Admin Key')).toBeVisible();
  });
});

test.describe('Multi-Index — Sidebar + Keys Workflow', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
  });

  test('should navigate from sidebar index to keys page and filter by index', async ({ page }) => {
    await page.goto('/');

    // See indices in sidebar
    await expect(page.getByTestId('sidebar-index-acme-products')).toBeVisible();

    // Navigate to API Keys page
    await page.getByRole('link', { name: /api keys/i }).click();
    await expect(page).toHaveURL(/\/keys/);

    // Verify filter bar shows index names
    await expect(page.getByTestId('index-filter-bar')).toBeVisible();
    await expect(page.getByTestId('filter-index-acme-products')).toBeVisible();
    await expect(page.getByTestId('filter-index-globex-inventory')).toBeVisible();

    // All 4 keys visible initially
    await expect(page.getByTestId('key-card')).toHaveCount(4);

    // Filter by Acme — should show 3 keys (Acme-scoped + global + cross-index)
    await page.getByTestId('filter-index-acme-products').click();
    await expect(page.getByTestId('key-card')).toHaveCount(3);
    await expect(page.getByText('Globex Full Access')).not.toBeVisible();

    // Reset filter
    await page.getByTestId('filter-all').click();
    await expect(page.getByTestId('key-card')).toHaveCount(4);
  });
});
