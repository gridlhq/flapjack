import { test, expect } from '../fixtures/auth.fixture';
import type { Page, Locator } from '@playwright/test';

/**
 * Dashboard UI Enhancement Tests
 *
 * Tests UI components:
 * - InfoTooltip help icons across the dashboard
 * - Index Health Cards on Overview page
 * - Enhanced System page index health with tooltips & clickable links
 * - System page Indices tab with summary cards, table, and pending tasks
 * - API Keys page scoping tooltips, filter bar, and actual filtering logic
 * - Key card scoping display (Shield vs Globe icons)
 * - Sidebar indices tooltip and expand/collapse
 * - Progressive disclosure: health cards hidden for single-index setups
 */

// ---------------------------------------------------------------------------
// Mock data
// ---------------------------------------------------------------------------

const SINGLE_INDEX = {
  items: [
    { uid: 'products', name: 'products', createdAt: '2024-01-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 5000, dataSize: 1048576, numberOfPendingTasks: 0 },
  ],
};

const MULTI_INDICES = {
  items: [
    { uid: 'acme-products', name: 'acme-products', createdAt: '2024-01-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 500, dataSize: 25000, numberOfPendingTasks: 0 },
    { uid: 'globex-inventory', name: 'globex-inventory', createdAt: '2024-02-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 200, dataSize: 10000, numberOfPendingTasks: 2 },
    { uid: 'shared-catalog', name: 'shared-catalog', createdAt: '2024-03-01T00:00:00Z', updatedAt: '2024-06-15T00:00:00Z', entries: 1000, dataSize: 50000, numberOfPendingTasks: 0 },
  ],
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
    value: 'key-globex-rw',
    description: 'Globex Read-Write',
    acl: ['search', 'addObject'],
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
];

const MOCK_HEALTH = {
  status: 'ok',
  active_writers: 2,
  max_concurrent_writers: 8,
  facet_cache_entries: 150,
  facet_cache_cap: 1000,
};

const MOCK_INTERNAL = {
  node_id: 'fj-node-abc123',
  replication_enabled: false,
  peer_count: 0,
};

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

async function mockApis(page: Page, indicesResponse = MULTI_INDICES, keysResponse = MOCK_KEYS) {
  await page.route('**/health', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
  });
  await page.route('**/2/overview**', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ totalSearches: 0, uniqueUsers: 0, noResultRate: 0, dates: [], indices: [] }),
    });
  });
  await page.route('**/internal/status', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
  });
  await page.route('**/1/keys', (route) => {
    if (route.request().method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ keys: keysResponse }),
      });
    }
    return route.fallback();
  });
  await page.route(
    (url: URL) => url.pathname === '/1/indexes',
    (route) => {
      if (route.request().method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(indicesResponse),
        });
      }
      return route.fallback();
    },
  );
}

/**
 * Hover a scoped tooltip trigger, verify the tooltip appears with expected text,
 * and verify the trigger has the correct aria-label for accessibility.
 */
async function hoverAndExpectTooltip(page: Page, trigger: Locator, textPattern: RegExp) {
  await expect(trigger).toHaveAttribute('aria-label', 'More information');
  await trigger.hover();
  const tooltip = page.getByRole('tooltip');
  await expect(tooltip).toBeVisible();
  await expect(tooltip).toContainText(textPattern);
}

// ===========================================================================
// InfoTooltip Component — Overview Page
// ===========================================================================

test.describe('InfoTooltip — Overview Page', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page);
    await page.goto('/overview');
  });

  test('shows help icon on Indices stat card with correct aria-label', async ({ page }) => {
    const trigger = page.getByTestId('stat-card-indices').getByTestId('info-tooltip-trigger');
    await expect(trigger).toBeVisible();
    await expect(trigger).toHaveAttribute('aria-label', 'More information');
  });

  test('shows help icon on Status stat card', async ({ page }) => {
    const trigger = page.getByTestId('stat-card-status').getByTestId('info-tooltip-trigger');
    await expect(trigger).toBeVisible();
  });

  test('Indices tooltip shows explanatory text on hover', async ({ page }) => {
    const trigger = page.getByTestId('stat-card-indices').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /isolated data container/i);
  });

  test('Status tooltip shows explanatory text on hover', async ({ page }) => {
    const trigger = page.getByTestId('stat-card-status').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /overall health/i);
  });
});

// ===========================================================================
// Index Health Cards
// ===========================================================================

test.describe('Index Health Cards — Multi-Index', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/overview');
  });

  test('shows Index Health section when multiple indices exist', async ({ page }) => {
    const section = page.getByTestId('index-health-section');
    await expect(section).toBeVisible();
    await expect(section).toContainText('Index Health');
  });

  test('shows a card for each index with name, doc count, and storage', async ({ page }) => {
    const acmeCard = page.getByTestId('index-card-acme-products');
    await expect(acmeCard).toBeVisible();
    await expect(acmeCard).toContainText('acme-products');
    await expect(acmeCard).toContainText('500 docs');
    // 25000 bytes → "24.4 KB"
    await expect(acmeCard).toContainText('KB');

    const globexCard = page.getByTestId('index-card-globex-inventory');
    await expect(globexCard).toContainText('globex-inventory');
    await expect(globexCard).toContainText('200 docs');

    const catalogCard = page.getByTestId('index-card-shared-catalog');
    await expect(catalogCard).toContainText('shared-catalog');
    await expect(catalogCard).toContainText('1,000 docs');
  });

  test('healthy index shows green status dot (no pending tasks)', async ({ page }) => {
    const acmeStatus = page.getByTestId('index-status-acme-products');
    await expect(acmeStatus).toBeVisible();
    await expect(acmeStatus).toHaveClass(/bg-green-500/);
    // Should NOT have the pulse animation
    await expect(acmeStatus).not.toHaveClass(/animate-pulse/);
  });

  test('index with pending tasks shows amber pulsing status dot', async ({ page }) => {
    const globexStatus = page.getByTestId('index-status-globex-inventory');
    await expect(globexStatus).toBeVisible();
    await expect(globexStatus).toHaveClass(/bg-amber-500/);
    await expect(globexStatus).toHaveClass(/animate-pulse/);
  });

  test('shows pending task count on degraded index card', async ({ page }) => {
    const globexCard = page.getByTestId('index-card-globex-inventory');
    await expect(globexCard).toContainText('2 pending');
  });

  test('healthy index card does NOT show pending text', async ({ page }) => {
    const acmeCard = page.getByTestId('index-card-acme-products');
    await expect(acmeCard).toBeVisible();
    await expect(acmeCard).not.toContainText('pending');
  });

  test('clicking an index card navigates to that index', async ({ page }) => {
    await page.getByTestId('index-card-acme-products').click();
    await expect(page).toHaveURL(/\/index\/acme-products/);
  });

  test('Index Health section has InfoTooltip with correct content', async ({ page }) => {
    const trigger = page.getByTestId('index-health-section').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /summary of each index/i);
  });
});

test.describe('Index Health Cards — Single-Index', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, SINGLE_INDEX);
    await page.goto('/overview');
  });

  test('hides Index Health section when only one index exists', async ({ page }) => {
    await expect(page.getByTestId('stat-card-indices')).toBeVisible();
    await expect(page.getByTestId('index-health-section')).not.toBeVisible();
  });
});

// ===========================================================================
// System Page — Index Health Summary
// ===========================================================================

test.describe('System Page — Index Health Summary', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/system');
  });

  test('shows index health summary card', async ({ page }) => {
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toBeVisible();
    await expect(summary).toContainText('Index Health');
  });

  test('shows correct healthy count "2 of 3 indices healthy"', async ({ page }) => {
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toContainText('2 of 3 indices healthy');
  });

  test('shows pending task count in summary', async ({ page }) => {
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toContainText('2 pending task(s)');
  });

  test('Index Health has InfoTooltip with correct content', async ({ page }) => {
    const trigger = page.getByTestId('index-health-summary').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /health status of each index/i);
  });

  test('index names are clickable links with correct href', async ({ page }) => {
    const acmeDot = page.getByTestId('index-dot-acme-products');
    await expect(acmeDot).toBeVisible();
    await expect(acmeDot).toHaveAttribute('href', /\/index\/acme-products/);

    const globexDot = page.getByTestId('index-dot-globex-inventory');
    await expect(globexDot).toHaveAttribute('href', /\/index\/globex-inventory/);
  });

  test('clicking index link in health summary navigates to index page', async ({ page }) => {
    await page.getByTestId('index-dot-acme-products').click();
    await expect(page).toHaveURL(/\/index\/acme-products/);
  });
});

// ===========================================================================
// System Page — Indices Tab
// ===========================================================================

test.describe('System Page — Indices Tab', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/system');
    await page.getByRole('tab', { name: /indices/i }).click();
  });

  test('shows summary cards with correct totals', async ({ page }) => {
    // Total Indices: 3
    await expect(page.getByTestId('indices-total-count').getByText('3')).toBeVisible();
    // Total Documents: 500 + 200 + 1000 = 1,700
    await expect(page.getByTestId('indices-total-docs').getByText('1,700')).toBeVisible();
    // Total Storage: 25000 + 10000 + 50000 = 85000 bytes ≈ 83 KB
    await expect(page.getByTestId('indices-total-storage')).toBeVisible();
  });

  test('shows pending tasks alert when tasks are pending', async ({ page }) => {
    // 2 pending from globex-inventory
    await expect(page.getByText('2 pending task(s) across indices')).toBeVisible();
  });

  test('shows Index Details table with all indices', async ({ page }) => {
    await expect(page.getByText('Index Details')).toBeVisible();
    // Verify all 3 indices appear in the table
    await expect(page.getByTestId('index-link-acme-products')).toBeVisible();
    await expect(page.getByTestId('index-link-globex-inventory')).toBeVisible();
    await expect(page.getByTestId('index-link-shared-catalog')).toBeVisible();
  });

  test('healthy index shows "Healthy" status in table', async ({ page }) => {
    const acmeStatus = page.getByTestId('index-status-acme-products');
    await expect(acmeStatus).toContainText('Healthy');
  });

  test('index with pending tasks shows "Processing" status in table', async ({ page }) => {
    const globexStatus = page.getByTestId('index-status-globex-inventory');
    await expect(globexStatus).toContainText('Processing (2)');
  });

  test('index names in table are links to index detail pages', async ({ page }) => {
    const acmeLink = page.getByTestId('index-link-acme-products');
    await expect(acmeLink).toHaveAttribute('href', /\/index\/acme-products/);
  });

  test('Index Details has InfoTooltip', async ({ page }) => {
    const detailsHeader = page.getByText('Index Details').locator('..');
    const trigger = detailsHeader.getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /isolated search collection/i);
  });
});

// ===========================================================================
// API Keys Page — Scoping Display
// ===========================================================================

test.describe('API Keys Page — Key Card Scoping', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/keys');
    await expect(page.getByTestId('keys-list')).toBeVisible();
  });

  test('scoped key shows its restricted index as a badge', async ({ page }) => {
    // "Acme Search Key" is scoped to acme-products
    const acmeCard = page.getByTestId('key-card').filter({ hasText: 'Acme Search Key' });
    const scopeSection = acmeCard.getByTestId('key-scope');
    await expect(scopeSection).toContainText('acme-products');
  });

  test('global key shows "All Indices" badge', async ({ page }) => {
    const globalCard = page.getByTestId('key-card').filter({ hasText: 'Global Admin Key' });
    const scopeSection = globalCard.getByTestId('key-scope');
    await expect(scopeSection).toContainText('All Indices');
  });

  test('scoped key scope section has InfoTooltip', async ({ page }) => {
    const trigger = page.getByTestId('key-card').first().getByTestId('key-scope').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /restricting a key/i);
  });

  test('key card shows permissions as badges', async ({ page }) => {
    const globalCard = page.getByTestId('key-card').filter({ hasText: 'Global Admin Key' });
    await expect(globalCard).toContainText('search');
    await expect(globalCard).toContainText('addObject');
    await expect(globalCard).toContainText('deleteObject');
    await expect(globalCard).toContainText('settings');
    await expect(globalCard).toContainText('deleteIndex');
  });

  test('key card displays key value in monospace', async ({ page }) => {
    const acmeCard = page.getByTestId('key-card').filter({ hasText: 'Acme Search Key' });
    await expect(acmeCard.locator('code')).toContainText('key-acme-search');
  });
});

// ===========================================================================
// API Keys Page — Filter Bar Behavior
// ===========================================================================

test.describe('API Keys Page — Filter Bar', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/keys');
    await expect(page.getByTestId('keys-list')).toBeVisible();
  });

  test('filter bar shows help text', async ({ page }) => {
    await expect(page.getByTestId('filter-help-text')).toContainText('Select an index to see which API keys');
  });

  test('filter bar has InfoTooltip', async ({ page }) => {
    const trigger = page.getByTestId('index-filter-bar').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /filter keys by which index/i);
  });

  test('"All" filter shows all keys', async ({ page }) => {
    // By default "All" is selected — all 3 keys visible
    const keyCards = page.getByTestId('key-card');
    await expect(keyCards).toHaveCount(3);
  });

  test('filtering by acme-products shows scoped key AND global key', async ({ page }) => {
    await page.getByTestId('filter-index-acme-products').click();

    // Acme key (scoped to acme-products) + Global key (no scope = all) should be visible
    // Globex key (scoped to globex-inventory) should be hidden
    const keyCards = page.getByTestId('key-card');
    await expect(keyCards).toHaveCount(2);
    await expect(page.getByTestId('key-card').filter({ hasText: 'Acme Search Key' })).toBeVisible();
    await expect(page.getByTestId('key-card').filter({ hasText: 'Global Admin Key' })).toBeVisible();
    await expect(page.getByTestId('key-card').filter({ hasText: 'Globex Read-Write' })).not.toBeVisible();
  });

  test('filtering by globex-inventory shows globex key AND global key', async ({ page }) => {
    await page.getByTestId('filter-index-globex-inventory').click();

    const keyCards = page.getByTestId('key-card');
    await expect(keyCards).toHaveCount(2);
    await expect(page.getByTestId('key-card').filter({ hasText: 'Globex Read-Write' })).toBeVisible();
    await expect(page.getByTestId('key-card').filter({ hasText: 'Global Admin Key' })).toBeVisible();
    await expect(page.getByTestId('key-card').filter({ hasText: 'Acme Search Key' })).not.toBeVisible();
  });

  test('clicking the same filter again deselects it (shows all keys)', async ({ page }) => {
    // Select acme-products
    await page.getByTestId('filter-index-acme-products').click();
    await expect(page.getByTestId('key-card')).toHaveCount(2);

    // Click acme-products again to deselect → shows all keys
    await page.getByTestId('filter-index-acme-products').click();
    await expect(page.getByTestId('key-card')).toHaveCount(3);
  });

  test('clicking "All" resets filter to show all keys', async ({ page }) => {
    await page.getByTestId('filter-index-acme-products').click();
    await expect(page.getByTestId('key-card')).toHaveCount(2);

    await page.getByTestId('filter-all').click();
    await expect(page.getByTestId('key-card')).toHaveCount(3);
  });
});

// ===========================================================================
// API Keys Page — Create Key Dialog
// ===========================================================================

test.describe('API Keys Page — Create Key Dialog Tooltip', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/keys');
  });

  test('Create Key dialog Index Scope section has InfoTooltip', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const scopeSection = dialog.getByTestId('index-scope-section');
    await expect(scopeSection).toBeVisible();
    await expect(scopeSection.getByTestId('info-tooltip-trigger')).toBeVisible();
  });

  test('Create Key dialog Index Scope tooltip shows explanatory text', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    const trigger = dialog.getByTestId('index-scope-section').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /restricts what data/i);
  });

  test('Create Key dialog shows index buttons for scoping', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    const scopeSection = dialog.getByTestId('index-scope-section');

    // All 3 indices should appear as scope buttons
    await expect(scopeSection.getByRole('button', { name: 'acme-products' })).toBeVisible();
    await expect(scopeSection.getByRole('button', { name: 'globex-inventory' })).toBeVisible();
    await expect(scopeSection.getByRole('button', { name: 'shared-catalog' })).toBeVisible();
  });

  test('selecting indices shows scope summary', async ({ page }) => {
    await page.getByRole('button', { name: /create key/i }).first().click();
    const dialog = page.getByRole('dialog');
    const scopeSection = dialog.getByTestId('index-scope-section');

    // Click an index to scope the key
    await scopeSection.getByRole('button', { name: 'acme-products' }).click();

    // Scope summary should appear showing selected index
    const summary = dialog.getByTestId('scope-summary');
    await expect(summary).toBeVisible();
    await expect(summary).toContainText('acme-products');
  });
});

// ===========================================================================
// Sidebar — Indices Section
// ===========================================================================

test.describe('Sidebar — Indices Section', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, MULTI_INDICES);
    await page.goto('/overview');
  });

  test('shows Indices header with InfoTooltip', async ({ page }) => {
    const header = page.getByTestId('sidebar-indices-header');
    await expect(header).toBeVisible();
    await expect(header).toContainText('Indices');
  });

  test('sidebar tooltip shows explanatory text on hover', async ({ page }) => {
    const trigger = page.getByTestId('sidebar-indices-header').getByTestId('info-tooltip-trigger');
    await hoverAndExpectTooltip(page, trigger, /isolated search collection/i);
  });

  test('shows all indices in sidebar when count <= 5', async ({ page }) => {
    // 3 indices, all should be visible (under MAX_VISIBLE_INDICES=5)
    await expect(page.getByTestId('sidebar-index-acme-products')).toBeVisible();
    await expect(page.getByTestId('sidebar-index-globex-inventory')).toBeVisible();
    await expect(page.getByTestId('sidebar-index-shared-catalog')).toBeVisible();
    // "Show all" button should NOT be visible since 3 <= 5
    await expect(page.getByTestId('sidebar-show-all-indices')).not.toBeVisible();
  });

  test('clicking a sidebar index navigates to that index', async ({ page }) => {
    await page.getByTestId('sidebar-index-acme-products').click();
    await expect(page).toHaveURL(/\/index\/acme-products/);
  });
});

test.describe('Sidebar — Expand/Collapse with many indices', () => {
  const MANY_INDICES = {
    items: Array.from({ length: 8 }, (_, i) => ({
      uid: `idx-${String(i + 1).padStart(2, '0')}`,
      name: `idx-${String(i + 1).padStart(2, '0')}`,
      entries: 100,
      dataSize: 1000,
      numberOfPendingTasks: 0,
    })),
  };

  test.beforeEach(async ({ page }) => {
    await mockApis(page, MANY_INDICES);
    await page.goto('/overview');
  });

  test('shows only first 5 indices and "Show all" button', async ({ page }) => {
    await expect(page.getByTestId('sidebar-index-idx-01')).toBeVisible();
    await expect(page.getByTestId('sidebar-index-idx-05')).toBeVisible();
    // 6th index should be hidden
    await expect(page.getByTestId('sidebar-index-idx-06')).not.toBeVisible();

    const showAllBtn = page.getByTestId('sidebar-show-all-indices');
    await expect(showAllBtn).toBeVisible();
    await expect(showAllBtn).toContainText('Show all (8)');
  });

  test('clicking "Show all" reveals all indices, button changes to "Show less"', async ({ page }) => {
    await page.getByTestId('sidebar-show-all-indices').click();

    // All 8 should now be visible
    await expect(page.getByTestId('sidebar-index-idx-06')).toBeVisible();
    await expect(page.getByTestId('sidebar-index-idx-08')).toBeVisible();

    // Button should now say "Show less"
    await expect(page.getByTestId('sidebar-show-all-indices')).toContainText('Show less');
  });

  test('clicking "Show less" collapses back to 5', async ({ page }) => {
    // Expand
    await page.getByTestId('sidebar-show-all-indices').click();
    await expect(page.getByTestId('sidebar-index-idx-08')).toBeVisible();

    // Collapse
    await page.getByTestId('sidebar-show-all-indices').click();
    await expect(page.getByTestId('sidebar-index-idx-06')).not.toBeVisible();
    await expect(page.getByTestId('sidebar-show-all-indices')).toContainText('Show all (8)');
  });
});

// ===========================================================================
// Progressive Disclosure — Features Hidden With Single Index
// ===========================================================================

test.describe('Progressive Disclosure — Single Index', () => {
  test.beforeEach(async ({ page }) => {
    await mockApis(page, SINGLE_INDEX);
  });

  test('Overview: no index health cards for single index', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indices')).toBeVisible();
    await expect(page.getByTestId('index-health-section')).not.toBeVisible();
  });

  test('Overview: stat card tooltips still show for single index (educational)', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByTestId('stat-card-indices').getByTestId('info-tooltip-trigger')).toBeVisible();
    await expect(page.getByTestId('stat-card-status').getByTestId('info-tooltip-trigger')).toBeVisible();
  });

  test('Sidebar: Indices section tooltip still shows for single index', async ({ page }) => {
    await page.goto('/overview');
    const header = page.getByTestId('sidebar-indices-header');
    await expect(header).toBeVisible();
    await expect(header.getByTestId('info-tooltip-trigger')).toBeVisible();
  });

  test('System: index health summary still shows for single index', async ({ page }) => {
    await page.goto('/system');
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toBeVisible();
    await expect(summary).toContainText('1 of 1 indices healthy');
  });
});

// ===========================================================================
// API Keys Page — Filter Bar Hidden When No Keys
// ===========================================================================

test.describe('API Keys Page — No Keys', () => {
  test('filter bar is hidden when there are no keys', async ({ page }) => {
    await mockApis(page, MULTI_INDICES, []);
    await page.goto('/keys');
    await expect(page.getByText('No API keys')).toBeVisible();
    await expect(page.getByTestId('index-filter-bar')).not.toBeVisible();
  });
});
