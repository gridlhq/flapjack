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
  replication_enabled: false,
  peer_count: 0,
};

const MOCK_INDICES = {
  results: [
    { uid: 'products', name: 'products', entries: 5000, dataSize: 1048576, numberOfPendingTasks: 0 },
    { uid: 'articles', name: 'articles', entries: 1200, dataSize: 524288, numberOfPendingTasks: 0 },
  ],
};

const MOCK_SNAPSHOTS = {
  snapshots: [
    { name: 'products-2026-02-01.tar.gz', size: 512000, lastModified: '2026-02-01T12:00:00Z' },
    { name: 'products-2026-02-05.tar.gz', size: 530000, lastModified: '2026-02-05T12:00:00Z' },
  ],
};

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

async function mockSystemApis(page: Page, opts: { s3Available?: boolean } = {}) {
  await page.route('**/health', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_HEALTH) });
  });
  await page.route('**/internal/status', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INTERNAL) });
  });
  await page.route((url) => url.pathname === '/1/indexes', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
  });

  // Snapshot list endpoint — S3 available or not
  await page.route('**/snapshots', (route) => {
    if (opts.s3Available) {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SNAPSHOTS) });
    } else {
      route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"S3 not configured"}' });
    }
  });

  // Export endpoint
  await page.route('**/export', (route) => {
    route.fulfill({ status: 200, contentType: 'application/octet-stream', body: Buffer.from('mock-tar-gz-content') });
  });

  // Import endpoint
  await page.route('**/import', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: '{"status":"ok"}' });
  });

  // Snapshot (backup) endpoint
  await page.route('**/snapshot', (route) => {
    if (route.request().method() === 'POST') {
      route.fulfill({ status: 200, contentType: 'application/json', body: '{"status":"ok"}' });
    } else {
      route.fallback();
    }
  });

  // Restore endpoint
  await page.route('**/restore', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: '{"status":"ok"}' });
  });
}

async function mockOverviewApis(page: Page) {
  await page.route('**/health', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
  });
  await page.route('**/2/overview**', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ totalSearches: 100, uniqueUsers: 10, noResultRate: 0.05, dates: [], indices: [] }),
    });
  });
  await page.route('**/1/indexes/*', (route) => {
    if (route.request().method() === 'DELETE') {
      route.fulfill({ status: 200, contentType: 'application/json', body: '{}' });
    } else {
      route.fallback();
    }
  });
  await page.route((url: URL) => url.pathname === '/1/indexes', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
  });
  // Export endpoint
  await page.route('**/export', (route) => {
    route.fulfill({ status: 200, contentType: 'application/octet-stream', body: Buffer.from('mock-tar-gz') });
  });
  // Import endpoint
  await page.route('**/import', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: '{"status":"ok"}' });
  });
}

// ===========================================================================
// System Page — Snapshots Tab
// ===========================================================================

test.describe('System Page — Snapshots Tab', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page);
    await page.goto('/system');
  });

  test('shows Snapshots tab', async ({ page }) => {
    await expect(page.getByRole('tab', { name: /snapshots/i })).toBeVisible();
  });

  test('clicking Snapshots tab shows snapshot content', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('snapshots-tab')).toBeVisible();
  });

  test('shows Local Export / Import section', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByText('Local Export / Import')).toBeVisible();
  });

  test('shows Export All button', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('export-all-btn')).toBeVisible();
  });

  test('shows Export and Import buttons for each index', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    await expect(page.getByTestId('export-btn-products')).toBeVisible();
    await expect(page.getByTestId('import-btn-products')).toBeVisible();
    await expect(page.getByTestId('export-btn-articles')).toBeVisible();
    await expect(page.getByTestId('import-btn-articles')).toBeVisible();
  });

  test('shows index name and doc count in snapshot rows', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    const productsRow = page.getByTestId('snapshot-index-products');
    await expect(productsRow).toContainText('products');
    await expect(productsRow).toContainText('5,000 docs');

    const articlesRow = page.getByTestId('snapshot-index-articles');
    await expect(articlesRow).toContainText('articles');
    await expect(articlesRow).toContainText('1,200 docs');
  });

  test('clicking Export triggers download request', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    const exportPromise = page.waitForResponse(
      (response) => response.url().includes('/export') && response.status() === 200,
    );

    await page.getByTestId('export-btn-products').click();
    await exportPromise;
  });

  test('shows hidden file input for imports', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    const fileInput = page.getByTestId('snapshot-file-input');
    await expect(fileInput).toBeAttached();
  });
});

// ===========================================================================
// System Page — Snapshots Tab (S3 Not Configured)
// ===========================================================================

test.describe('System Page — Snapshots Tab (S3 Not Configured)', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page, { s3Available: false });
    await page.goto('/system');
  });

  test('shows S3 not configured message', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('s3-not-configured')).toBeVisible();
    await expect(page.getByText(/FLAPJACK_S3_BUCKET/)).toBeVisible();
  });

  test('does not show Backup All to S3 button when S3 not configured', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('s3-not-configured')).toBeVisible();
    await expect(page.getByTestId('backup-all-s3-btn')).not.toBeVisible();
  });
});

// ===========================================================================
// System Page — Snapshots Tab (S3 Available)
// ===========================================================================

test.describe('System Page — Snapshots Tab (S3 Available)', () => {
  test.beforeEach(async ({ page }) => {
    await mockSystemApis(page, { s3Available: true });
    await page.goto('/system');
  });

  test('shows S3 Backups section with Backup All button', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByTestId('s3-section')).toBeVisible();
    await expect(page.getByTestId('backup-all-s3-btn')).toBeVisible();
  });

  test('shows Backup and Restore buttons per index', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    await expect(page.getByTestId('backup-btn-products')).toBeVisible();
    await expect(page.getByTestId('restore-btn-products')).toBeVisible();
    await expect(page.getByTestId('backup-btn-articles')).toBeVisible();
    await expect(page.getByTestId('restore-btn-articles')).toBeVisible();
  });

  test('shows snapshot count text', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByText(/2 snapshot\(s\) available/)).toBeVisible();
  });

  test('clicking Backup sends POST to snapshot endpoint', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    const snapshotPromise = page.waitForResponse(
      (response) =>
        response.url().includes('/products/snapshot') &&
        response.request().method() === 'POST' &&
        response.status() === 200,
    );

    await page.getByTestId('backup-btn-products').click();
    await snapshotPromise;
  });

  test('clicking Restore sends POST to restore endpoint', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();

    const restorePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/products/restore') &&
        response.request().method() === 'POST' &&
        response.status() === 200,
    );

    await page.getByTestId('restore-btn-products').click();
    await restorePromise;
  });
});

// ===========================================================================
// System Page — Snapshots Tab (No Indices)
// ===========================================================================

test.describe('System Page — Snapshots Tab (No Indices)', () => {
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

  test('shows empty state when no indices exist', async ({ page }) => {
    await page.getByRole('tab', { name: /snapshots/i }).click();
    await expect(page.getByText(/no indices available/i)).toBeVisible();
  });
});

// ===========================================================================
// Overview Page — Export/Import Buttons
// ===========================================================================

test.describe('Overview Page — Export/Import Buttons', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');
  });

  test('shows Export All button in header', async ({ page }) => {
    await expect(page.getByTestId('overview-export-all-btn')).toBeVisible();
    await expect(page.getByTestId('overview-export-all-btn')).toContainText('Export All');
  });

  test('shows Export and Import buttons on each index row', async ({ page }) => {
    await expect(page.getByText('products').first()).toBeVisible();

    await expect(page.getByTestId('overview-export-products')).toBeVisible();
    await expect(page.getByTestId('overview-import-products')).toBeVisible();
    await expect(page.getByTestId('overview-export-articles')).toBeVisible();
    await expect(page.getByTestId('overview-import-articles')).toBeVisible();
  });

  test('clicking Export on an index triggers download request', async ({ page }) => {
    await expect(page.getByText('products').first()).toBeVisible();

    const exportPromise = page.waitForResponse(
      (response) => response.url().includes('/export') && response.status() === 200,
    );

    await page.getByTestId('overview-export-products').click();
    await exportPromise;
  });

  test('shows hidden file input for imports', async ({ page }) => {
    const fileInput = page.getByTestId('overview-file-input');
    await expect(fileInput).toBeAttached();
  });
});
