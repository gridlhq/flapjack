import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

// ---------------------------------------------------------------------------
// Mock data
// ---------------------------------------------------------------------------

const MOCK_HEALTH = {
  status: 'ok',
  active_writers: 2,
  max_concurrent_writers: 8,
  facet_cache_entries: 150,
  facet_cache_cap: 1000,
};

const MOCK_INTERNAL = {
  node_id: 'fj-node-abc123',
  replication_enabled: true,
  peer_count: 3,
  ssl_renewal: {
    certificate_expiry: '2027-01-15T00:00:00Z',
    next_renewal: '2026-12-15T00:00:00Z',
  },
};

const MOCK_INDICES = {
  results: [
    { uid: 'products', name: 'products', entries: 5000, dataSize: 1048576, numberOfPendingTasks: 0 },
    { uid: 'articles', name: 'articles', entries: 1200, dataSize: 524288, numberOfPendingTasks: 2 },
  ],
};

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

async function mockSystemApis(page: Page) {
  await page.route('**/health', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
  });
  await page.route('**/internal/status', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
  });
  await page.route((url) => url.pathname === '/1/indexes', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
  });
}

async function mockHealthError(page: Page) {
  await page.route('**/health', (route) => {
    route.fulfill({ status: 500, contentType: 'application/json', body: JSON.stringify({ error: 'Internal Server Error' }) });
  });
}

async function mockNonHealthApis(page: Page) {
  await page.route('**/internal/status', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
  });
  await page.route((url) => url.pathname === '/1/indexes', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
  });
}

// ---------------------------------------------------------------------------
// Tests — Health Tab (default)
// ---------------------------------------------------------------------------

test.describe('System Page — Health Tab', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page);
    await page.goto('/system');
  });

  test('shows System heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'System' })).toBeVisible();
  });

  test('shows Health tab selected by default', async ({ page }) => {
    const healthTab = page.getByRole('tab', { name: /health/i });
    await expect(healthTab).toBeVisible();
    await expect(healthTab).toHaveAttribute('data-state', 'active');
  });

  test('shows Indices and Replication tabs', async ({ page }) => {
    await expect(page.getByRole('tab', { name: /indices/i })).toBeVisible();
    await expect(page.getByRole('tab', { name: /replication/i })).toBeVisible();
  });

  test('displays "ok" status indicator', async ({ page }) => {
    const statusCard = page.getByTestId('health-status');
    await expect(statusCard).toBeVisible();
    await expect(statusCard).toContainText('ok');
  });

  test('displays active writers count "2" and max "8"', async ({ page }) => {
    const writersCard = page.getByTestId('health-active-writers');
    await expect(writersCard).toBeVisible();
    await expect(writersCard).toContainText('2 / 8');
  });

  test('displays facet cache entries "150" and cap "1000"', async ({ page }) => {
    const cacheCard = page.getByTestId('health-facet-cache');
    await expect(cacheCard).toBeVisible();
    await expect(cacheCard).toContainText('150 / 1000');
  });

  test('shows index health summary card', async ({ page }) => {
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toBeVisible();
    await expect(summary).toContainText('Index Health');
  });

  test('shows dot indicators per index with correct health colors', async ({ page }) => {
    const productsDot = page.getByTestId('index-dot-products');
    await expect(productsDot).toBeVisible();
    // products has 0 pending tasks — green dot
    await expect(productsDot.locator('span.rounded-full')).toHaveClass(/bg-green-500/);

    const articlesDot = page.getByTestId('index-dot-articles');
    await expect(articlesDot).toBeVisible();
    // articles has 2 pending tasks — amber pulsing dot
    await expect(articlesDot.locator('span.rounded-full')).toHaveClass(/bg-amber-500/);
  });

  test('shows index health summary text with pending tasks', async ({ page }) => {
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toContainText('1 of 2 indices healthy');
    await expect(summary).toContainText('2 pending task(s)');
  });
});

// ---------------------------------------------------------------------------
// Tests — Health Tab Error
// ---------------------------------------------------------------------------

test.describe('System Page — Health Tab Error', () => {
  test.beforeEach(async ({ page }) => {
    await mockHealthError(page);
    await mockNonHealthApis(page);
    await page.goto('/system');
  });

  test('shows error message about failed health fetch', async ({ page }) => {
    await expect(page.getByText(/failed to fetch health status/i)).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — Health Tab (All Healthy)
// ---------------------------------------------------------------------------

test.describe('System Page — Index Health All Healthy', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
    });
    await page.route('**/internal/status', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          results: [
            { uid: 'products', name: 'products', entries: 5000, dataSize: 1048576, numberOfPendingTasks: 0 },
            { uid: 'articles', name: 'articles', entries: 1200, dataSize: 524288, numberOfPendingTasks: 0 },
          ],
        }),
      });
    });
    await page.goto('/system');
  });

  test('shows all indices as healthy when none have pending tasks', async ({ page }) => {
    const summary = page.getByTestId('index-health-summary');
    await expect(summary).toContainText('2 of 2 indices healthy');
  });
});

// ---------------------------------------------------------------------------
// Tests — Indices Tab
// ---------------------------------------------------------------------------

test.describe('System Page — Indices Tab', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page);
    await page.goto('/system');
  });

  test('clicking Indices tab shows index information', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await expect(page.getByText('Total Indices')).toBeVisible();
    await expect(page.getByText('Index Details')).toBeVisible();
  });

  test('displays total indices count (2)', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const card = page.getByTestId('indices-total-count');
    await expect(card).toBeVisible();
    await expect(card).toContainText('2');
  });

  test('displays total documents count (6,200)', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const card = page.getByTestId('indices-total-docs');
    await expect(card).toBeVisible();
    await expect(card).toContainText('6,200');
  });

  test('displays total storage', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const card = page.getByTestId('indices-total-storage');
    await expect(card).toBeVisible();
    // 1048576 + 524288 = 1572864 bytes = 1.5 MB
    await expect(card).toContainText('1.5 MB');
  });

  test('shows index detail table with "products" and "articles"', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await expect(page.getByRole('cell', { name: 'products' })).toBeVisible();
    await expect(page.getByRole('cell', { name: 'articles' })).toBeVisible();
  });

  test('shows document counts (5,000 and 1,200) in table', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await expect(page.getByText('5,000')).toBeVisible();
    await expect(page.getByText('1,200')).toBeVisible();
  });

  test('shows pending tasks indicator for articles (2 pending)', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await expect(page.getByText(/2 pending task\(s\) across indices/)).toBeVisible();
  });

  test('shows Status column header', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await expect(page.getByRole('columnheader', { name: 'Status' })).toBeVisible();
  });

  test('shows "Healthy" status for products (0 pending)', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const status = page.getByTestId('index-status-products');
    await expect(status).toBeVisible();
    await expect(status).toContainText('Healthy');
  });

  test('shows "Processing (2)" status for articles (2 pending)', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const status = page.getByTestId('index-status-articles');
    await expect(status).toBeVisible();
    await expect(status).toContainText('Processing (2)');
  });

  test('index names are clickable links', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const link = page.getByTestId('index-link-products');
    await expect(link).toBeVisible();
    await expect(link).toHaveAttribute('href', '/index/products');
  });

  test('clicking index name navigates to index page', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await page.getByTestId('index-link-products').click();
    await expect(page).toHaveURL(/\/index\/products/);
  });
});

// ---------------------------------------------------------------------------
// Tests — Indices Tab Empty
// ---------------------------------------------------------------------------

test.describe('System Page — Indices Tab Empty', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
    });
    await page.route('**/internal/status', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });
    await page.goto('/system');
  });

  test('clicking Indices tab shows zero indices', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    const card = page.getByTestId('indices-total-count');
    await expect(card).toBeVisible();
    await expect(card).toContainText('0');
  });
});

// ---------------------------------------------------------------------------
// Tests — Replication Tab
// ---------------------------------------------------------------------------

test.describe('System Page — Replication Tab', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page);
    await page.goto('/system');
  });

  test('clicking Replication tab shows node information', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('Node ID')).toBeVisible();
  });

  test('displays node ID "fj-node-abc123"', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('fj-node-abc123')).toBeVisible();
  });

  test('shows replication is Enabled', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('Enabled')).toBeVisible();
  });

  test('shows peer count (3)', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('3 peer(s) connected')).toBeVisible();
  });

  test('shows SSL certificate expiry date', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('SSL / TLS')).toBeVisible();
    await expect(page.getByText(/Certificate expires:/)).toBeVisible();
    await expect(page.getByText('2027-01-15T00:00:00Z')).toBeVisible();
  });

  test('shows next renewal date', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText(/Next renewal:/)).toBeVisible();
    await expect(page.getByText('2026-12-15T00:00:00Z')).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — Replication Disabled
// ---------------------------------------------------------------------------

test.describe('System Page — Replication Disabled', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
    });
    await page.route('**/internal/status', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ node_id: 'fj-node-xyz', replication_enabled: false, peer_count: 0 }),
      });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
    });
    await page.goto('/system');
  });

  test('shows replication is Disabled', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('Disabled')).toBeVisible();
  });

  test('does not show peer count text', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    // Wait for Disabled to confirm the tab content has loaded
    await expect(page.getByText('Disabled')).toBeVisible();
    await expect(page.getByText(/peer\(s\) connected/)).not.toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — Navigation
// ---------------------------------------------------------------------------

test.describe('System Page — Navigation', () => {
  test('accessible from sidebar navigation', async ({ page }) => {
    await mockSystemApis(page);
    await page.goto('/overview');
    await page.getByRole('link', { name: /system/i }).click();
    await expect(page).toHaveURL(/\/system/);
    await expect(page.getByRole('heading', { name: 'System' })).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — Indices Tab Error State
// ---------------------------------------------------------------------------

test.describe('System Page — Indices Tab Error', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
    });
    await page.route('**/internal/status', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"Internal Server Error"}' });
    });
    await page.goto('/system');
  });

  test('shows "Unable to load indices" error message', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();
    await expect(page.getByText('Unable to load indices.')).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — Replication Tab Error State
// ---------------------------------------------------------------------------

test.describe('System Page — Replication Tab Error', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
    });
    await page.route('**/internal/status', (route) => {
      route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"Internal Server Error"}' });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
    });
    await page.goto('/system');
  });

  test('shows "Replication status unavailable" error message', async ({ page }) => {
    await page.getByRole('tab', { name: /replication/i }).click();
    await expect(page.getByText('Replication status unavailable')).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — IndexHealthSummary Hidden When Empty
// ---------------------------------------------------------------------------

test.describe('System Page — Index Health Hidden When Empty', () => {
  test.beforeEach(async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
    });
    await page.route('**/internal/status', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
    });
    await page.route((url) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) });
    });
    await page.goto('/system');
  });

  test('index health summary is hidden when indices list is empty', async ({ page }) => {
    // Ensure health tab content loaded
    await expect(page.getByTestId('health-status')).toBeVisible();

    // IndexHealthSummary should NOT be visible
    await expect(page.getByTestId('index-health-summary')).not.toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Tests — Per-Index Storage Size Formatting
// ---------------------------------------------------------------------------

test.describe('System Page — Storage Size Formatting', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page);
    await page.goto('/system');
  });

  test('shows formatted storage sizes per index in detail table', async ({ page }) => {
    await page.getByRole('tab', { name: /indices/i }).click();

    // products: 1048576 bytes = 1 MB
    await expect(page.getByText('1 MB')).toBeVisible();
    // articles: 524288 bytes = 512 KB
    await expect(page.getByText('512 KB')).toBeVisible();
  });
});
