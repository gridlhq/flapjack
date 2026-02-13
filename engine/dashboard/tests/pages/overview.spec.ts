import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

// ---------------------------------------------------------------------------
// Mock data
// ---------------------------------------------------------------------------

const MOCK_INDICES = {
  results: [
    { uid: 'products', name: 'products', entries: 5000, dataSize: 1048576, updatedAt: '2026-02-07T12:00:00Z', numberOfPendingTasks: 0 },
    { uid: 'articles', name: 'articles', entries: 1200, dataSize: 524288, updatedAt: '2026-02-06T08:30:00Z', numberOfPendingTasks: 0 },
    { uid: 'users', name: 'users', entries: 300, dataSize: 102400, updatedAt: '2026-02-05T15:00:00Z', numberOfPendingTasks: 0 },
  ],
};

const MOCK_ANALYTICS = {
  totalSearches: 15432,
  uniqueUsers: 2341,
  noResultRate: 0.054,
  dates: [
    { date: '2026-02-01', count: 2100 },
    { date: '2026-02-02', count: 2300 },
    { date: '2026-02-03', count: 2050 },
    { date: '2026-02-04', count: 2400 },
    { date: '2026-02-05', count: 2180 },
    { date: '2026-02-06', count: 2200 },
    { date: '2026-02-07', count: 2202 },
  ],
  indices: [
    { index: 'products', searches: 10000 },
    { index: 'articles', searches: 5432 },
  ],
};

const MANY_INDICES = {
  results: Array.from({ length: 15 }, (_, i) => ({
    uid: `index-${String(i + 1).padStart(2, '0')}`,
    name: `index-${String(i + 1).padStart(2, '0')}`,
    entries: (i + 1) * 100,
    dataSize: (i + 1) * 10000,
    updatedAt: '2026-02-07T12:00:00Z',
    numberOfPendingTasks: 0,
  })),
};

// ---------------------------------------------------------------------------
// Mock helper
// ---------------------------------------------------------------------------

async function mockOverviewApis(page: Page, indicesResponse = MOCK_INDICES) {
  // Health
  await page.route('**/health', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ status: 'ok' }),
    });
  });

  // Analytics overview
  await page.route('**/2/overview**', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(MOCK_ANALYTICS),
    });
  });

  // Delete index — glob matches /1/indexes/{name}, must be registered BEFORE
  // the exact-pathname handler so DELETE on /1/indexes/products is caught here.
  await page.route('**/1/indexes/*', (route) => {
    if (route.request().method() === 'DELETE') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: '{}',
      });
    } else {
      route.fallback();
    }
  });

  // List indices (GET) and Create index (POST) — exact pathname match
  await page.route(
    (url: URL) => url.pathname === '/1/indexes',
    (route) => {
      if (route.request().method() === 'POST') {
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ uid: 'new-index' }),
        });
      } else {
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(indicesResponse),
        });
      }
    },
  );
}

// ===========================================================================
// Overview Page — Empty State
// ===========================================================================

test.describe('Overview Page — Empty State', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page, { results: [] });
    await page.goto('/overview');
  });

  test('shows Overview heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible();
  });

  test('shows stat cards with zero values', async ({ page }) => {
    await expect(page.getByTestId('stat-card-indices').getByText('0')).toBeVisible();
    await expect(page.getByTestId('stat-card-documents').getByText('0')).toBeVisible();
  });

  test('shows "no indices" empty state message', async ({ page }) => {
    await expect(page.getByText(/no indices/i)).toBeVisible();
  });

  test('shows Create Index button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /create.*index/i })).toBeVisible();
  });

  test('does not show pagination', async ({ page }) => {
    await expect(page.getByText(/no indices/i)).toBeVisible();
    await expect(page.getByText(/Showing/)).not.toBeVisible();
    await expect(page.getByRole('button', { name: /Previous/ })).not.toBeVisible();
    await expect(page.getByRole('button', { name: /Next/ })).not.toBeVisible();
  });
});

// ===========================================================================
// Overview Page — With Indices
// ===========================================================================

test.describe('Overview Page — With Indices', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');
  });

  test('shows 4 stat cards', async ({ page }) => {
    await expect(page.getByTestId('stat-card-indices')).toBeVisible();
    await expect(page.getByTestId('stat-card-documents')).toBeVisible();
    await expect(page.getByTestId('stat-card-storage')).toBeVisible();
    await expect(page.getByTestId('stat-card-status')).toBeVisible();
  });

  test('shows indices count of 3', async ({ page }) => {
    await expect(page.getByTestId('stat-card-indices').getByText('3')).toBeVisible();
  });

  test('shows total documents count of 6,500', async ({ page }) => {
    await expect(page.getByTestId('stat-card-documents').getByText('6,500')).toBeVisible();
  });

  test('shows Healthy status', async ({ page }) => {
    await expect(page.getByText('Healthy')).toBeVisible();
  });

  test('renders index list with products, articles, and users', async ({ page }) => {
    await expect(page.getByText('products').first()).toBeVisible();
    await expect(page.getByText('articles')).toBeVisible();
    await expect(page.getByText('users')).toBeVisible();
  });

  test('shows document count for each index', async ({ page }) => {
    await expect(page.getByText(/5,000 documents/)).toBeVisible();
    await expect(page.getByText(/1,200 documents/)).toBeVisible();
    await expect(page.getByText(/300 documents/)).toBeVisible();
  });

  test('shows storage size for each index', async ({ page }) => {
    // Scope to the index list section to avoid matching index health cards
    // products: 1048576 bytes = 1 MB
    await expect(page.getByText(/5,000 documents.*1 MB/)).toBeVisible();
    // articles: 524288 bytes = 512 KB
    await expect(page.getByText(/1,200 documents.*512 KB/)).toBeVisible();
    // users: 102400 bytes = 100 KB
    await expect(page.getByText(/300 documents.*100 KB/)).toBeVisible();
  });

  test('clicking an index row navigates to /index/{name}', async ({ page }) => {
    await page.getByText('products').first().click();
    await expect(page).toHaveURL(/\/index\/products/);
  });
});

// ===========================================================================
// Overview Page — Analytics Section
// ===========================================================================

test.describe('Overview Page — Analytics Section', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');
  });

  test('shows analytics section with data-testid="overview-analytics"', async ({ page }) => {
    await expect(page.locator('[data-testid="overview-analytics"]')).toBeVisible();
  });

  test('shows total searches count of 15,432', async ({ page }) => {
    const analyticsSection = page.locator('[data-testid="overview-analytics"]');
    await expect(analyticsSection.getByText('15,432')).toBeVisible();
    await expect(analyticsSection.getByText('Total Searches')).toBeVisible();
  });

  test('shows unique users count of 2,341', async ({ page }) => {
    const analyticsSection = page.locator('[data-testid="overview-analytics"]');
    await expect(analyticsSection.getByText('2,341')).toBeVisible();
    await expect(analyticsSection.getByText('Unique Users')).toBeVisible();
  });

  test('shows no-result rate of 5.4%', async ({ page }) => {
    const analyticsSection = page.locator('[data-testid="overview-analytics"]');
    await expect(analyticsSection.getByText('5.4%')).toBeVisible();
    await expect(analyticsSection.getByText('No-Result Rate')).toBeVisible();
  });

  test('shows search volume chart with SVG area path', async ({ page }) => {
    const analyticsSection = page.locator('[data-testid="overview-analytics"]');
    // Recharts renders a <linearGradient id="overviewGradient"> and an <Area fill="url(#overviewGradient)">
    const gradient = analyticsSection.locator('linearGradient#overviewGradient');
    await expect(gradient).toBeAttached();
    // The Area component renders a <path> with the gradient fill
    const areaPath = analyticsSection.locator('path[fill="url(#overviewGradient)"]');
    await expect(areaPath).toBeAttached();
  });
});

// ===========================================================================
// Overview Page — Pagination
// ===========================================================================

test.describe('Overview Page — Pagination', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page, MANY_INDICES);
    await page.goto('/overview');
  });

  test('shows "Showing 1-10 of 15 indices"', async ({ page }) => {
    await expect(page.getByText('Showing 1-10 of 15 indices')).toBeVisible();
  });

  test('shows Previous and Next buttons', async ({ page }) => {
    await expect(page.getByRole('button', { name: /Previous/ })).toBeVisible();
    await expect(page.getByRole('button', { name: /Next/ })).toBeVisible();
  });

  test('Previous button is disabled on first page', async ({ page }) => {
    await expect(page.getByRole('button', { name: /Previous/ })).toBeDisabled();
  });

  test('clicking Next shows page 2 with remaining 5 indices', async ({ page }) => {
    await page.getByRole('button', { name: /Next/ }).click();
    await expect(page.getByText('Showing 11-15 of 15 indices')).toBeVisible();

    // Should see the last 5 indices (use headings to avoid sidebar ambiguity)
    await expect(page.getByRole('heading', { name: 'index-11' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'index-15' })).toBeVisible();

    // First-page items should no longer be visible in the main list
    await expect(page.getByRole('heading', { name: 'index-01' })).not.toBeVisible();
  });

  test('Next button is disabled on last page', async ({ page }) => {
    await page.getByRole('button', { name: /Next/ }).click();
    await expect(page.getByText('Showing 11-15 of 15 indices')).toBeVisible();
    await expect(page.getByRole('button', { name: /Next/ })).toBeDisabled();
  });

  test('clicking Previous returns to page 1', async ({ page }) => {
    // Go to page 2
    await page.getByRole('button', { name: /Next/ }).click();
    await expect(page.getByText('Showing 11-15 of 15 indices')).toBeVisible();

    // Go back to page 1
    await page.getByRole('button', { name: /Previous/ }).click();
    await expect(page.getByText('Showing 1-10 of 15 indices')).toBeVisible();
    await expect(page.getByRole('heading', { name: 'index-01' })).toBeVisible();
  });
});

// ===========================================================================
// Overview Page — Create Index Dialog
// ===========================================================================

test.describe('Overview Page — Create Index Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');
  });

  test('clicking Create Index opens dialog', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole('heading', { name: 'Create Index' })).toBeVisible();
  });

  test('dialog shows index name input', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.locator('#index-uid')).toBeVisible();
  });

  test('dialog shows template options (Empty, Movies, Products)', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText('Empty index')).toBeVisible();
    await expect(dialog.getByText(/movies/i)).toBeVisible();
    await expect(dialog.getByText(/products/i)).toBeVisible();
  });

  test('Cancel closes dialog', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();

    await page.getByRole('button', { name: /cancel/i }).click();
    await expect(page.getByRole('dialog')).not.toBeVisible();
  });

  test('creating an index sends POST to /1/indexes', async ({ page }) => {
    await page.getByRole('button', { name: /create.*index/i }).click();
    const dialog = page.getByRole('dialog');

    await dialog.locator('#index-uid').fill('my-new-index');

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/1/indexes') &&
        response.request().method() === 'POST' &&
        response.status() === 200,
    );

    await dialog.getByRole('button', { name: /create index/i }).click();

    const response = await responsePromise;
    const postData = response.request().postDataJSON();
    expect(postData.uid).toBe('my-new-index');
  });
});

// ===========================================================================
// Overview Page — Delete Index
// ===========================================================================

test.describe('Overview Page — Delete Index', () => {
  test.beforeEach(async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');
  });

  test('clicking delete on an index shows confirmation dialog', async ({ page }) => {
    // Wait for index list to render
    await expect(page.getByText('products').first()).toBeVisible();

    const deleteButton = page.getByRole('button', { name: /delete.*products/i });
    await deleteButton.click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/are you sure/i)).toBeVisible();
    await expect(dialog.getByText('products')).toBeVisible();
  });

  test('confirming delete sends DELETE to /1/indexes/{name}', async ({ page }) => {
    await expect(page.getByText('products').first()).toBeVisible();

    await page.getByRole('button', { name: /delete.*products/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/1/indexes/products') &&
        response.request().method() === 'DELETE' &&
        response.status() === 200,
    );

    await dialog.getByRole('button', { name: /delete/i }).click();
    await responsePromise;
  });
});

// ===========================================================================
// Overview Page — Navigation
// ===========================================================================

test.describe('Overview Page — Navigation', () => {
  test('clicking Settings on an index navigates to /index/{name}/settings', async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');

    await expect(page.getByText('products').first()).toBeVisible();

    // The Settings button is a link inside the index row
    const settingsLinks = page.getByRole('link', { name: /settings/i });
    await settingsLinks.first().click();
    await expect(page).toHaveURL(/\/index\/products\/settings/);
  });
});

// ===========================================================================
// Overview Page — Health Status Variants
// ===========================================================================

test.describe('Overview Page — Disconnected Health Status', () => {
  test('shows "Disconnected" when health endpoint fails', async ({ page }) => {
    // Health returns error
    await page.route('**/health', (route) => {
      route.fulfill({ status: 500, contentType: 'application/json', body: '{}' });
    });
    // Analytics
    await page.route('**/2/overview**', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_ANALYTICS) });
    });
    // Indices
    await page.route((url: URL) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
    });

    await page.goto('/overview');

    // useHealth retries twice with exponential backoff before erroring,
    // so "Disconnected" takes a few seconds to appear
    const statusCard = page.getByTestId('stat-card-status');
    await expect(statusCard.getByText('Disconnected')).toBeVisible({ timeout: 15000 });
  });
});

// ===========================================================================
// Overview Page — Error Loading Indices
// ===========================================================================

test.describe('Overview Page — Error Loading Indices', () => {
  test('shows error message when indices endpoint fails', async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
    });
    await page.route('**/2/overview**', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_ANALYTICS) });
    });
    await page.route((url: URL) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 500, contentType: 'application/json', body: JSON.stringify({ error: 'DB error' }) });
    });

    await page.goto('/overview');

    await expect(page.getByText(/error loading indices/i)).toBeVisible();
  });
});

// ===========================================================================
// Overview Page — Analytics Hidden When Zero Searches
// ===========================================================================

test.describe('Overview Page — Analytics Hidden When Zero', () => {
  test('hides analytics section when totalSearches is 0', async ({ page }) => {
    await page.route('**/health', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
    });
    await page.route('**/2/overview**', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ totalSearches: 0, uniqueUsers: 0, noResultRate: 0, dates: [], indices: [] }),
      });
    });
    await page.route((url: URL) => url.pathname === '/1/indexes', (route) => {
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
    });

    await page.goto('/overview');

    // Wait for index list to render (meaning data loaded)
    await expect(page.getByText('products').first()).toBeVisible();

    // Analytics section should NOT be visible
    await expect(page.locator('[data-testid="overview-analytics"]')).not.toBeVisible();
  });
});

// ===========================================================================
// Overview Page — Storage Stat Card
// ===========================================================================

test.describe('Overview Page — Storage Stat', () => {
  test('shows correct total storage (1.6 MB from 3 indices)', async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');

    // 1048576 + 524288 + 102400 = 1675264 bytes = ~1.6 MB
    const storageCard = page.getByTestId('stat-card-storage');
    await expect(storageCard).toBeVisible();
    await expect(storageCard.getByText('1.6 MB')).toBeVisible();
  });
});

// ===========================================================================
// Overview Page — Analytics View Details Link
// ===========================================================================

test.describe('Overview Page — Analytics View Details', () => {
  test('shows "View Details" link to first index analytics', async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');

    const viewDetails = page.getByRole('link', { name: /view details/i });
    await expect(viewDetails).toBeVisible();
    await expect(viewDetails).toHaveAttribute('href', '/index/products/analytics');
  });
});

// ===========================================================================
// Overview Page — Index Distribution Text
// ===========================================================================

test.describe('Overview Page — Index Distribution Text', () => {
  test('shows search breakdown across indices', async ({ page }) => {
    await mockOverviewApis(page);
    await page.goto('/overview');

    await expect(page.getByText(/across 2 indices/i)).toBeVisible();
    await expect(page.getByText(/products \(10000\)/)).toBeVisible();
    await expect(page.getByText(/articles \(5432\)/)).toBeVisible();
  });
});
