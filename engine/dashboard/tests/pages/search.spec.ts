import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

// ─── Mock Data ──────────────────────────────────────────────────────────────

const MOCK_SEARCH = {
  hits: [
    { objectID: 'prod-1', title: 'Blue Widget', category: 'Widgets', price: 9.99, _highlightResult: { title: { value: 'Blue <em>Widget</em>' } } },
    { objectID: 'prod-2', title: 'Red Gadget', category: 'Gadgets', price: 19.99, _highlightResult: { title: { value: 'Red Gadget' } } },
    { objectID: 'prod-3', title: 'Green Gizmo', category: 'Gizmos', price: 14.99, _highlightResult: { title: { value: 'Green Gizmo' } } },
  ],
  nbHits: 3,
  page: 0,
  nbPages: 1,
  hitsPerPage: 20,
  processingTimeMS: 2,
  query: '',
  facets: { category: { Widgets: 1, Gadgets: 1, Gizmos: 1 } },
};

const EMPTY_SEARCH = {
  hits: [],
  nbHits: 0,
  page: 0,
  nbPages: 0,
  hitsPerPage: 20,
  processingTimeMS: 1,
  query: 'nonexistent',
  facets: {},
};

const MOCK_INDEX_META = {
  results: [
    { uid: 'test-index', name: 'test-index', entries: 1234, dataSize: 56789 },
  ],
};

// ─── Mock Helpers ───────────────────────────────────────────────────────────

async function mockSearchApi(page: Page, response = MOCK_SEARCH) {
  await page.route('**/1/indexes/*/query', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(response) });
  });
}

async function mockIndicesApi(page: Page) {
  await page.route((url) => url.pathname === '/1/indexes', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDEX_META) });
  });
}

async function mockHealthApi(page: Page) {
  await page.route('**/health', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
  });
}

async function mockEventsApi(page: Page) {
  await page.route('**/1/events', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ message: 'ok' }) });
  });
}

// ─── Search Page — With Results ─────────────────────────────────────────────

test.describe('Search Page — With Results', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');
  });

  test('shows index name "test-index" in heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'test-index' })).toBeVisible();
  });

  test('shows search input', async ({ page }) => {
    await expect(page.getByPlaceholder(/search documents/i)).toBeVisible();
  });

  test('shows search button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /^search$/i })).toBeVisible();
  });

  test('shows breadcrumb with Overview link', async ({ page }) => {
    await expect(page.getByRole('button', { name: /overview/i })).toBeVisible();
  });

  test('displays search results with objectIDs', async ({ page }) => {
    const resultsPanel = page.locator('[data-testid="results-panel"]');
    await expect(resultsPanel).toBeVisible();

    await expect(page.getByText('prod-1')).toBeVisible();
    await expect(page.getByText('prod-2')).toBeVisible();
    await expect(page.getByText('prod-3')).toBeVisible();
  });

  test('shows result count and processing time', async ({ page }) => {
    // The results header has count and time in separate spans within a text-sm div
    const header = page.locator('[data-testid="results-panel"] .text-sm').first();
    await expect(header).toContainText('3');
    await expect(header).toContainText('results');
    await expect(header).toContainText('2ms');
  });

  test('shows facets panel with category facet', async ({ page }) => {
    const facetsPanel = page.locator('[data-testid="facets-panel"]');
    await expect(facetsPanel).toBeVisible();

    await expect(facetsPanel.getByText('category')).toBeVisible();
    await expect(facetsPanel.getByText('Widgets')).toBeVisible();
    await expect(facetsPanel.getByText('Gadgets')).toBeVisible();
    await expect(facetsPanel.getByText('Gizmos')).toBeVisible();
  });

  test('shows Add Documents button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /add documents/i })).toBeVisible();
  });

  test('shows Synonyms navigation link', async ({ page }) => {
    await expect(page.getByRole('link', { name: /synonyms/i })).toBeVisible();
  });

  test('shows Merchandising navigation link', async ({ page }) => {
    await expect(page.getByRole('link', { name: /merchandising/i })).toBeVisible();
  });

  test('shows Analytics navigation link', async ({ page }) => {
    await expect(page.getByRole('link', { name: /analytics/i })).toBeVisible();
  });

  test('shows Settings navigation link', async ({ page }) => {
    await expect(page.getByRole('link', { name: /settings/i })).toBeVisible();
  });
});

// ─── Search Page — Empty Results ────────────────────────────────────────────

test.describe('Search Page — Empty Results', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page, EMPTY_SEARCH);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');
  });

  test('shows "no results" message or empty state', async ({ page }) => {
    await expect(page.getByText(/no results found/i)).toBeVisible();
  });
});

// ─── Search Page — Search Execution ─────────────────────────────────────────

test.describe('Search Page — Search Execution', () => {
  test('typing in search input and pressing Enter sends query to API', async ({ page }) => {
    let lastBody: any = null;

    await page.route('**/1/indexes/*/query', (route) => {
      lastBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ ...MOCK_SEARCH, query: lastBody?.query || '' }),
      });
    });
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);

    await page.goto('/index/test-index');

    // Wait for initial search to complete (page loads with empty query)
    await expect(page.locator('[data-testid="results-panel"]')).toBeVisible();

    const searchInput = page.getByPlaceholder(/search documents/i);
    await searchInput.fill('widget');

    const responsePromise = page.waitForResponse((resp) =>
      resp.url().includes('/1/indexes/') && resp.url().includes('/query') && resp.status() === 200
    );
    await searchInput.press('Enter');
    await responsePromise;

    expect(lastBody).toBeTruthy();
    expect(lastBody.query).toBe('widget');
  });

  test('search results update after new query', async ({ page }) => {
    let queryCount = 0;
    const updatedSearch = {
      ...MOCK_SEARCH,
      hits: [
        { objectID: 'prod-99', title: 'Special Widget', category: 'Widgets', price: 29.99, _highlightResult: { title: { value: 'Special <em>Widget</em>' } } },
      ],
      nbHits: 1,
      query: 'special',
    };

    await page.route('**/1/indexes/*/query', (route) => {
      queryCount++;
      const body = route.request().postDataJSON();
      const response = body?.query === 'special' ? updatedSearch : MOCK_SEARCH;
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(response),
      });
    });
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);

    await page.goto('/index/test-index');

    // Wait for initial results
    await expect(page.getByText('prod-1')).toBeVisible();

    // Perform a new search
    const searchInput = page.getByPlaceholder(/search documents/i);
    await searchInput.fill('special');

    await searchInput.press('Enter');

    // New results should be visible (auto-retries until element appears)
    await expect(page.getByText('prod-99')).toBeVisible();
    await expect(page.getByText('Special Widget')).toBeVisible();
  });
});

// ─── Search Page — Add Documents Dialog ─────────────────────────────────────

test.describe('Search Page — Add Documents Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');
  });

  test('clicking Add Documents opens dialog', async ({ page }) => {
    await page.getByRole('button', { name: /add documents/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
  });

  test('dialog has JSON tab', async ({ page }) => {
    await page.getByRole('button', { name: /add documents/i }).click();
    await expect(page.getByRole('tab', { name: /json/i })).toBeVisible();
  });

  test('dialog has Upload tab', async ({ page }) => {
    await page.getByRole('button', { name: /add documents/i }).click();
    await expect(page.getByRole('tab', { name: /upload/i })).toBeVisible();
  });

  test('dialog has Sample Data tab', async ({ page }) => {
    await expect(page.getByRole('button', { name: /add documents/i })).toBeVisible();
    await page.getByRole('button', { name: /add documents/i }).click();
    await expect(page.getByRole('tab', { name: /sample data/i })).toBeVisible();
  });

  test('JSON tab shows Add Field button', async ({ page }) => {
    await page.getByRole('button', { name: /add documents/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('button', { name: /add field/i })).toBeVisible();
  });

  test('Sample Data tab shows Load Movies button', async ({ page }) => {
    await page.getByRole('button', { name: /add documents/i }).click();
    await page.getByRole('tab', { name: /sample data/i }).click();
    await expect(page.getByRole('button', { name: /load.*movies/i })).toBeVisible();
  });
});

// ─── Search Page — Navigation ───────────────────────────────────────────────

test.describe('Search Page — Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');
  });

  test('clicking Overview breadcrumb navigates to /overview', async ({ page }) => {
    await page.getByRole('button', { name: /overview/i }).click();
    await expect(page).toHaveURL(/\/overview/);
  });

  test('clicking Analytics link navigates to /index/test-index/analytics', async ({ page }) => {
    await page.getByRole('link', { name: /analytics/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index\/analytics/);
  });

  test('clicking Settings link navigates to /index/test-index/settings', async ({ page }) => {
    await page.getByRole('link', { name: /settings/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index\/settings/);
  });

  test('clicking Synonyms link navigates to /index/test-index/synonyms', async ({ page }) => {
    await page.getByRole('link', { name: /synonyms/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index\/synonyms/);
  });

  test('clicking Merchandising link navigates to /index/test-index/merchandising', async ({ page }) => {
    await page.getByRole('link', { name: /merchandising/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index\/merchandising/);
  });
});

// ─── Search Page — Analytics Tracking Toggle ─────────────────────────────────

test.describe('Search Page — Analytics Toggle', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');
  });

  test('shows Track Analytics toggle', async ({ page }) => {
    await expect(page.locator('#track-analytics')).toBeVisible();
    await expect(page.getByText('Track Analytics')).toBeVisible();
  });

  test('analytics toggle is off by default', async ({ page }) => {
    const toggle = page.locator('#track-analytics');
    await expect(toggle).toBeVisible();
    // Switch should not be checked (data-state="unchecked")
    await expect(toggle).toHaveAttribute('data-state', 'unchecked');
  });

  test('toggling analytics on shows red pulsing dot', async ({ page }) => {
    await page.locator('#track-analytics').click();

    // The red pulsing dot should appear near the label
    const label = page.getByText('Track Analytics');
    const dot = label.locator('..').locator('svg.animate-pulse, .animate-pulse');
    await expect(dot).toBeVisible();
  });

  test('search with analytics on sends analytics params', async ({ page }) => {
    let capturedBody: any = null;
    await page.route('**/1/indexes/*/query', (route) => {
      capturedBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_SEARCH),
      });
    });

    await page.locator('#track-analytics').click();

    const searchInput = page.getByPlaceholder(/search documents/i);
    await searchInput.fill('test');

    const responsePromise = page.waitForResponse(
      (resp) => resp.url().includes('/query') && resp.status() === 200,
    );
    await searchInput.press('Enter');
    await responsePromise;

    expect(capturedBody).toBeTruthy();
    expect(capturedBody.analytics).toBe(true);
    expect(capturedBody.clickAnalytics).toBe(true);
    expect(capturedBody.analyticsTags).toContain('source:dashboard');
  });
});

// ─── Search Page — Pagination ────────────────────────────────────────────────

test.describe('Search Page — Pagination', () => {
  test('shows pagination controls for multi-page results', async ({ page }) => {
    const multiPageSearch = {
      ...MOCK_SEARCH,
      nbHits: 50,
      nbPages: 3,
      hitsPerPage: 20,
    };
    await mockSearchApi(page, multiPageSearch);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    await expect(page.locator('[data-testid="results-panel"]')).toBeVisible();
    // Should show pagination - next/prev or page numbers
    await expect(page.getByRole('button', { name: /next/i }).or(page.getByText(/page/i))).toBeVisible();
  });
});

// ─── Search Page — Facet Interaction ────────────────────────────────────────

test.describe('Search Page — Facet Interaction', () => {
  test('clicking a facet value re-executes search with facetFilters', async ({ page }) => {
    const requests: any[] = [];
    await page.route('**/1/indexes/*/query', (route) => {
      const body = route.request().postDataJSON();
      requests.push(body);
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_SEARCH),
      });
    });
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    // Wait for facets panel
    const facetsPanel = page.locator('[data-testid="facets-panel"]');
    await expect(facetsPanel).toBeVisible();
    await expect(facetsPanel.getByText('Widgets')).toBeVisible();

    const initialCount = requests.length;

    // Click a facet value
    await facetsPanel.locator('button', { hasText: 'Widgets' }).click();

    // Wait for at least one new request after the click
    await expect.poll(() => requests.length).toBeGreaterThan(initialCount);

    // Check that a post-click request includes facetFilters
    const postClickRequests = requests.slice(initialCount);
    const hasFacetFilter = postClickRequests.some(
      (body) =>
        body?.facetFilters?.some?.((f: any) =>
          typeof f === 'string' ? f.includes('category:Widgets') : Array.isArray(f) && f.some((s: string) => s.includes('category:Widgets'))
        ) || (typeof body?.filters === 'string' && body.filters.includes('category'))
    );
    expect(hasFacetFilter).toBe(true);
  });
});

// ─── Search Page — Document Deletion ─────────────────────────────────────────

test.describe('Search Page — Document Deletion', () => {
  test('clicking delete button on a result opens confirm dialog', async ({ page }) => {
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    const resultsPanel = page.locator('[data-testid="results-panel"]');
    await expect(resultsPanel).toBeVisible();

    // Click the delete button (Trash2 icon) on the first result card
    const deleteBtn = resultsPanel.getByRole('button', { name: /delete/i }).first();
    await deleteBtn.click();

    // Confirm dialog should appear
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText('Delete Document')).toBeVisible();
    await expect(dialog.getByText('prod-1')).toBeVisible();
    await expect(dialog.getByRole('button', { name: /^delete$/i })).toBeVisible();
  });

  test('confirming delete sends DELETE request to API', async ({ page }) => {
    let deleteUrl = '';
    await page.route('**/1/indexes/test-index/*', (route) => {
      if (route.request().method() === 'DELETE') {
        deleteUrl = route.request().url();
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ taskID: 42 }),
        });
      } else {
        route.fallback();
      }
    });
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    const resultsPanel = page.locator('[data-testid="results-panel"]');
    await expect(resultsPanel).toBeVisible();

    // Click delete on first result
    await resultsPanel.getByRole('button', { name: /delete/i }).first().click();

    // Confirm
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: /^delete$/i }).click();

    await expect.poll(() => deleteUrl).toContain('/1/indexes/test-index/prod-1');
  });

  test('dismissing delete confirm does not send DELETE', async ({ page }) => {
    let deleteRequested = false;
    page.on('request', (req) => {
      if (req.method() === 'DELETE' && req.url().includes('/1/indexes/test-index/')) {
        deleteRequested = true;
      }
    });
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    const resultsPanel = page.locator('[data-testid="results-panel"]');
    await expect(resultsPanel).toBeVisible();

    // Click delete
    await resultsPanel.getByRole('button', { name: /delete/i }).first().click();

    // Cancel the dialog
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: /cancel/i }).click();

    // Wait for dialog to close — confirms the dismiss action completed
    await expect(dialog).not.toBeVisible();
    expect(deleteRequested).toBe(false);
  });
});

// ─── Search Page — Facets Panel Filter Input ─────────────────────────────────

test.describe('Search Page — Facets Panel Filter', () => {
  test('typing in facets filter input narrows displayed facet values', async ({ page }) => {
    const multiFacetSearch = {
      ...MOCK_SEARCH,
      facets: {
        category: { Widgets: 5, Gadgets: 3, Gizmos: 2 },
        brand: { Acme: 4, Globex: 2 },
      },
    };
    await mockSearchApi(page, multiFacetSearch);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    const facetsPanel = page.locator('[data-testid="facets-panel"]');
    await expect(facetsPanel).toBeVisible();

    // All facet values should be visible initially
    await expect(facetsPanel.getByText('Widgets')).toBeVisible();
    await expect(facetsPanel.getByText('Gadgets')).toBeVisible();
    await expect(facetsPanel.getByText('Acme')).toBeVisible();

    // Type in the filter input
    await facetsPanel.getByPlaceholder('Filter facets...').fill('wid');

    // Only "Widgets" should match
    await expect(facetsPanel.getByText('Widgets')).toBeVisible();
    await expect(facetsPanel.getByText('Gadgets')).not.toBeVisible();
    await expect(facetsPanel.getByText('Acme')).not.toBeVisible();
  });
});

// ─── Search Page — Search Error State ───────────────────────────────────────

test.describe('Search Page — Error State', () => {
  test('does not crash when search API returns 500', async ({ page }) => {
    await page.route('**/1/indexes/*/query', (route) => {
      route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"Internal Server Error"}' });
    });
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    // Page should still render the heading (not crash)
    await expect(page.getByRole('heading', { name: 'test-index' })).toBeVisible();
    // No results should be displayed
    await expect(page.getByText('prod-1')).not.toBeVisible();
  });
});

// ─── Search Page — Index Stats Display ───────────────────────────────────────

test.describe('Search Page — Index Stats', () => {
  test('shows index data size and document count', async ({ page }) => {
    await mockSearchApi(page);
    await mockIndicesApi(page);
    await mockHealthApi(page);
    await mockEventsApi(page);
    await page.goto('/index/test-index');

    // MOCK_INDEX_META has entries: 1234, dataSize: 56789 → 55.46 KB
    await expect(page.getByText('55.46 KB')).toBeVisible();
    await expect(page.getByText('1,234 docs')).toBeVisible();
  });
});
