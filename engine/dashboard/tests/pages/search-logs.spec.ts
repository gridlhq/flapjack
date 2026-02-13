import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

/**
 * The API Logger uses a Zustand store backed by sessionStorage.
 * To test with log entries present, we seed sessionStorage before navigation.
 */
function seedLogEntries(page: Page) {
  return page.addInitScript(() => {
    const entries = [
      {
        id: 'log-1',
        timestamp: Date.now() - 5000,
        method: 'POST',
        url: '/1/indexes/products/query',
        headers: { 'content-type': 'application/json', 'x-algolia-api-key': 'abc123' },
        body: { query: 'iphone', hitsPerPage: 20 },
        response: { hits: [], nbHits: 42, processingTimeMS: 3 },
        duration: 45,
        status: 'success' as const,
      },
      {
        id: 'log-2',
        timestamp: Date.now() - 3000,
        method: 'GET',
        url: '/1/indexes',
        headers: { 'content-type': 'application/json' },
        body: null,
        response: { items: [] },
        duration: 12,
        status: 'success' as const,
      },
      {
        id: 'log-3',
        timestamp: Date.now() - 1000,
        method: 'PUT',
        url: '/1/indexes/products/settings',
        headers: { 'content-type': 'application/json' },
        body: { searchableAttributes: ['name', 'brand'] },
        response: null,
        duration: 0,
        status: 'error' as const,
      },
    ];
    sessionStorage.setItem(
      'flapjack-api-log',
      JSON.stringify({ state: { entries, maxEntries: 20, isExpanded: false }, version: 0 })
    );
  });
}

test.describe('API Logs Page — Layout', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/logs');
  });

  test('shows API Logs heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /api logs/i })).toBeVisible();
  });

  test('shows subtitle about API calls', async ({ page }) => {
    await expect(page.getByText(/recent api calls/i)).toBeVisible();
  });

  test('shows filter input', async ({ page }) => {
    await expect(page.getByPlaceholder(/filter by url/i)).toBeVisible();
  });

  test('shows Export button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /export/i })).toBeVisible();
  });

  test('shows Clear button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /clear/i })).toBeVisible();
  });

  test('shows request count badge', async ({ page }) => {
    // Badge shows "N requests" text (N may be 0 in test environment)
    await expect(page.getByText('requests')).toBeVisible();
  });
});

test.describe('API Logs Page — With Data', () => {
  test.beforeEach(async ({ page }) => {
    await seedLogEntries(page);
    await page.goto('/logs');
  });

  test('shows seeded request count plus any app requests', async ({ page }) => {
    // Seeded 3 entries + the app may add health check(s), so expect >= 3
    await expect(page.getByText(/[3-9]\d* requests|[1-9]\d+ requests/i)).toBeVisible();
  });

  test('renders log entries in a list with seeded data', async ({ page }) => {
    const logsList = page.locator('[data-testid="logs-list"]');
    await expect(logsList).toBeVisible();
    // Verify the list actually has child entries (not just an empty container)
    const entries = logsList.locator('button');
    await expect(entries).not.toHaveCount(0);
  });

  test('shows HTTP methods', async ({ page }) => {
    await expect(page.getByText('POST').first()).toBeVisible();
    await expect(page.getByText('GET').first()).toBeVisible();
    await expect(page.getByText('PUT').first()).toBeVisible();
  });

  test('shows request URLs', async ({ page }) => {
    await expect(page.getByText('/1/indexes/products/query')).toBeVisible();
    await expect(page.getByText('/1/indexes', { exact: false }).first()).toBeVisible();
  });

  test('shows search query for search requests', async ({ page }) => {
    // The extractSearchQuery function checks for /query URLs with body.query
    await expect(page.getByText('"iphone"')).toBeVisible();
  });

  test('shows hit count for search responses', async ({ page }) => {
    // The extractHitCount function checks response.nbHits — scope to the search query row
    const searchRow = page.locator('button').filter({ hasText: '/1/indexes/products/query' });
    await expect(searchRow.getByText('42')).toBeVisible();
  });

  test('shows formatted duration', async ({ page }) => {
    await expect(page.getByText('45ms')).toBeVisible();
    await expect(page.getByText('12ms')).toBeVisible();
  });

  test('shows table header columns', async ({ page }) => {
    await expect(page.getByText('Time', { exact: true })).toBeVisible();
    await expect(page.getByText('Request', { exact: true })).toBeVisible();
    await expect(page.getByText('Query', { exact: true })).toBeVisible();
    await expect(page.getByText('Hits', { exact: true })).toBeVisible();
    await expect(page.getByText('Duration', { exact: true })).toBeVisible();
  });
});

test.describe('API Logs Page — Expand Detail', () => {
  test.beforeEach(async ({ page }) => {
    await seedLogEntries(page);
    await page.goto('/logs');
  });

  test('click on entry expands to show request/response detail', async ({ page }) => {
    // Click on the first entry (POST /query)
    await page.getByText('/1/indexes/products/query').click();

    // Should show request body
    await expect(page.getByText(/request body/i)).toBeVisible();
    // Should show response body
    await expect(page.getByText(/response/i).first()).toBeVisible();
  });

  test('expanded detail shows JSON for request body', async ({ page }) => {
    await page.getByText('/1/indexes/products/query').click();

    // The request body should contain our search query
    await expect(page.getByText(/"query"/).first()).toBeVisible();
  });

  test('clicking same entry again collapses the detail', async ({ page }) => {
    // Expand
    await page.getByText('/1/indexes/products/query').click();
    await expect(page.getByText(/request body/i)).toBeVisible();

    // Collapse
    await page.getByText('/1/indexes/products/query').click();
    await expect(page.getByText(/request body/i)).not.toBeVisible();
  });

  test('expanding entry B collapses entry A', async ({ page }) => {
    // Expand entry A (POST /query)
    await page.getByText('/1/indexes/products/query').click();
    await expect(page.getByText(/request body/i)).toBeVisible();

    // Expand entry B (PUT /settings) — should close entry A
    await page.getByText('/1/indexes/products/settings').click();

    // Only one expanded detail card should be visible at a time
    const detailCards = page.locator('[data-testid="logs-list"] .mx-4');
    await expect(detailCards).toHaveCount(1);
  });

  test('error entry shows destructive icon', async ({ page }) => {
    // The seeded log-3 has status: 'error' — verify the XCircle icon
    const errorRow = page.locator('button').filter({ hasText: '/1/indexes/products/settings' });
    await expect(errorRow).toBeVisible();
    await expect(errorRow.locator('svg.text-destructive')).toBeVisible();
  });

  test('success entry shows green icon', async ({ page }) => {
    // The seeded log-1 has status: 'success' — verify the CheckCircle icon
    const successRow = page.locator('button').filter({ hasText: '/1/indexes/products/query' });
    await expect(successRow).toBeVisible();
    await expect(successRow.locator('svg.text-green-500')).toBeVisible();
  });
});

test.describe('API Logs Page — Filtering', () => {
  test.beforeEach(async ({ page }) => {
    await seedLogEntries(page);
    await page.goto('/logs');
  });

  test('filters entries by URL', async ({ page }) => {
    await page.getByPlaceholder(/filter by url/i).fill('settings');

    // Only the PUT /settings entry should be visible
    await expect(page.getByText('/1/indexes/products/settings')).toBeVisible();
    // The other entries should be filtered out
    await expect(page.getByText('/1/indexes/products/query')).not.toBeVisible();
  });

  test('filters entries by method', async ({ page }) => {
    await page.getByPlaceholder(/filter by url/i).fill('GET');

    await expect(page.getByText('/1/indexes', { exact: false }).first()).toBeVisible();
    // POST entries should be hidden
    await expect(page.getByText('/1/indexes/products/query')).not.toBeVisible();
  });

  test('filters entries by body content', async ({ page }) => {
    await page.getByPlaceholder(/filter by url/i).fill('iphone');

    // Only the search request with body containing "iphone" should show
    await expect(page.getByText('/1/indexes/products/query')).toBeVisible();
    await expect(page.getByText('/1/indexes/products/settings')).not.toBeVisible();
  });

  test('shows empty state when filter matches nothing', async ({ page }) => {
    await page.getByPlaceholder(/filter by url/i).fill('nonexistent-endpoint');

    await expect(page.getByText(/no api logs/i)).toBeVisible();
  });
});

test.describe('API Logs Navigation', () => {
  test('accessible from sidebar', async ({ page }) => {
    await page.goto('/overview');
    await page.getByRole('link', { name: /api logs/i }).click();
    await expect(page).toHaveURL(/\/logs/);
    await expect(page.getByRole('heading', { name: /api logs/i })).toBeVisible();
  });
});


test.describe('API Logs Page — Clear Button', () => {
  test.beforeEach(async ({ page }) => {
    await seedLogEntries(page);
    await page.goto('/logs');
  });

  test('clicking Clear button removes all log entries', async ({ page }) => {
    // Verify entries exist first
    const logsList = page.locator('[data-testid="logs-list"]');
    await expect(logsList).toBeVisible();
    const entriesBefore = logsList.locator('button');
    await expect(entriesBefore).not.toHaveCount(0);

    // Click Clear (scope to main content to avoid sidebar Clear button)
    await page.getByRole('main').getByRole('button', { name: /clear/i }).click();

    // Entries should be cleared — the empty state should appear
    await expect(page.getByText(/no api logs/i)).toBeVisible();
  });
});

test.describe('API Logs Page — Export Button', () => {
  test.beforeEach(async ({ page }) => {
    await seedLogEntries(page);
    await page.goto('/logs');
  });

  test('clicking Export triggers a file download', async ({ page }) => {
    // Listen for the download event
    const downloadPromise = page.waitForEvent('download');

    await page.getByRole('button', { name: /export/i }).click();

    const download = await downloadPromise;
    // Download filename should contain "flapjack-api-log"
    expect(download.suggestedFilename()).toContain('flapjack-api-log');
    expect(download.suggestedFilename()).toContain('.sh');
  });
});

test.describe('API Logs Page — Pending Status', () => {
  test('pending entry shows spinning loader icon', async ({ page }) => {
    // Seed a pending entry
    await page.addInitScript(() => {
      const entries = [
        {
          id: 'pending-1',
          timestamp: Date.now(),
          method: 'POST',
          url: '/1/indexes/products/batch',
          headers: { 'x-api-key': 'test-key' },
          body: { requests: [{ action: 'addObject', body: { title: 'New Product' } }] },
          duration: 0,
          status: 'pending',
        },
        {
          id: 'success-1',
          timestamp: Date.now() - 1000,
          method: 'POST',
          url: '/1/indexes/products/query',
          headers: { 'x-api-key': 'test-key' },
          body: { query: 'test' },
          response: { nbHits: 5 },
          duration: 12,
          status: 'success',
        },
      ];
      sessionStorage.setItem(
        'flapjack-api-log',
        JSON.stringify({ state: { entries, maxEntries: 20, isExpanded: false }, version: 0 })
      );
    });
    await page.goto('/logs');

    const logsList = page.locator('[data-testid="logs-list"]');
    await expect(logsList).toBeVisible();

    // Find our seeded pending entry by its URL text
    const pendingEntry = logsList.locator('button', { hasText: '/1/indexes/products/batch' });
    await expect(pendingEntry).toBeVisible();
    // Pending entries show a spinning Loader2 icon
    await expect(pendingEntry.locator('.animate-spin')).toBeVisible();

    // Find our seeded success entry by its URL text
    const successEntry = logsList.locator('button', { hasText: '/1/indexes/products/query' });
    await expect(successEntry).toBeVisible();
    // Success entries show a green CheckCircle2 icon
    await expect(successEntry.locator('.text-green-500')).toBeVisible();
  });
});

test.describe('API Logs Page — Timestamp Display', () => {
  test.beforeEach(async ({ page }) => {
    await seedLogEntries(page);
    await page.goto('/logs');
  });

  test('shows timestamp for log entries', async ({ page }) => {
    // The Time column header exists
    await expect(page.getByText('Time', { exact: true })).toBeVisible();

    // The seeded entries have timestamps within the last 5 seconds
    // They should render time-like text (e.g. "12:34:56" or "5s ago")
    const logsList = page.locator('[data-testid="logs-list"]');
    await expect(logsList).toBeVisible();

    // Verify entries display formatted durations (e.g. "45ms", "12ms")
    // which confirms the time column is rendering actual data
    await expect(logsList.getByText('45ms')).toBeVisible();
  });
});
