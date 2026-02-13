import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

// ─── Mock Data ──────────────────────────────────────────────────────────────

const MOCK_SEARCH_COUNT = {
  count: 15420,
  dates: [
    { date: '2026-02-01', count: 2200 },
    { date: '2026-02-02', count: 2100 },
    { date: '2026-02-03', count: 2350 },
    { date: '2026-02-04', count: 2150 },
    { date: '2026-02-05', count: 2180 },
    { date: '2026-02-06', count: 2240 },
    { date: '2026-02-07', count: 2200 },
  ],
};

const MOCK_USERS_COUNT = {
  count: 3280,
  dates: [
    { date: '2026-02-01', count: 480 },
    { date: '2026-02-02', count: 460 },
    { date: '2026-02-03', count: 490 },
    { date: '2026-02-04', count: 470 },
    { date: '2026-02-05', count: 465 },
    { date: '2026-02-06', count: 455 },
    { date: '2026-02-07', count: 460 },
  ],
};

const MOCK_NO_RESULT_RATE = {
  rate: 0.045,
  dates: [
    { date: '2026-02-01', rate: 0.05 },
    { date: '2026-02-02', rate: 0.04 },
    { date: '2026-02-03', rate: 0.045 },
    { date: '2026-02-04', rate: 0.042 },
    { date: '2026-02-05', rate: 0.048 },
    { date: '2026-02-06', rate: 0.044 },
    { date: '2026-02-07', rate: 0.047 },
  ],
};

const MOCK_TOP_SEARCHES = {
  searches: [
    { search: 'wireless headphones', count: 1520, nbHits: 45 },
    { search: 'laptop stand', count: 980, nbHits: 23 },
    { search: 'usb-c cable', count: 870, nbHits: 67 },
    { search: 'mechanical keyboard', count: 650, nbHits: 12 },
    { search: 'monitor arm', count: 430, nbHits: 8 },
  ],
};

const MOCK_NO_RESULTS = {
  searches: [
    { search: 'unicorn widget', count: 42 },
    { search: 'nonexistent product', count: 18 },
    { search: 'xyzzy123', count: 5 },
  ],
};

const MOCK_TOP_FILTERS = {
  filters: [
    { attribute: 'brand:Apple', count: 3200 },
    { attribute: 'brand:Samsung', count: 2800 },
    { attribute: 'category:Electronics', count: 5100 },
  ],
};

const MOCK_FILTERS_NO_RESULTS = {
  filters: [
    { attribute: 'brand:Obsolete', count: 15 },
  ],
};

const MOCK_DEVICES = {
  platforms: [
    { platform: 'desktop', count: 9200 },
    { platform: 'mobile', count: 4600 },
    { platform: 'tablet', count: 1620 },
  ],
  dates: [
    { date: '2026-02-01', platform: 'desktop', count: 1300 },
    { date: '2026-02-01', platform: 'mobile', count: 650 },
    { date: '2026-02-01', platform: 'tablet', count: 230 },
  ],
};

const MOCK_GEO = {
  countries: [
    { country: 'US', count: 8500 },
    { country: 'GB', count: 2100 },
    { country: 'DE', count: 1800 },
  ],
  total: 15420,
};

const MOCK_GEO_TOP_SEARCHES = {
  searches: [
    { search: 'wireless headphones', count: 520 },
    { search: 'laptop stand', count: 380 },
  ],
};

const MOCK_GEO_REGIONS = {
  regions: [
    { region: 'California', count: 2100 },
    { region: 'New York', count: 1800 },
    { region: 'Texas', count: 1500 },
  ],
};

const MOCK_INDICES = {
  results: [
    { uid: 'test-index', name: 'test-index', entries: 5000, dataSize: 1048576 },
  ],
};

// ─── Mock Helpers ───────────────────────────────────────────────────────────

async function mockAnalyticsApis(page: Page) {
  await page.route((reqUrl) => reqUrl.pathname.startsWith('/2/'), (route) => {
    const pathname = new URL(route.request().url()).pathname;
    const method = route.request().method();

    // Mutations
    if (method === 'POST' && pathname === '/2/analytics/flush') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{"message":"ok"}' });
    }
    if (method === 'DELETE' && pathname === '/2/analytics/clear') {
      return route.fulfill({ status: 200, contentType: 'application/json', body: '{"message":"ok"}' });
    }

    let body: any = {};

    if (pathname === '/2/searches/count') body = MOCK_SEARCH_COUNT;
    else if (pathname === '/2/searches/noResultRate') body = MOCK_NO_RESULT_RATE;
    else if (pathname === '/2/searches/noResults') body = MOCK_NO_RESULTS;
    else if (pathname === '/2/searches') body = MOCK_TOP_SEARCHES;
    else if (pathname === '/2/users/count') body = MOCK_USERS_COUNT;
    else if (pathname === '/2/filters/noResults') body = MOCK_FILTERS_NO_RESULTS;
    else if (/^\/2\/filters\/[^/]+$/.test(pathname)) body = { values: [{ value: 'SomeValue', count: 50 }] };
    else if (pathname === '/2/filters') body = MOCK_TOP_FILTERS;
    else if (pathname === '/2/devices') body = MOCK_DEVICES;
    else if (/^\/2\/geo\/[^/]+\/regions$/.test(pathname)) body = MOCK_GEO_REGIONS;
    else if (/^\/2\/geo\/[^/]+$/.test(pathname)) body = MOCK_GEO_TOP_SEARCHES;
    else if (pathname === '/2/geo') body = MOCK_GEO;
    else if (pathname === '/2/overview') body = {};
    else if (pathname === '/2/status') body = { enabled: true };

    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
  });
}

async function mockIndicesApi(page: Page) {
  await page.route((reqUrl) => reqUrl.pathname === '/1/indexes', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_INDICES) });
  });
}

async function setupPage(page: Page) {
  await mockAnalyticsApis(page);
  await mockIndicesApi(page);
}

// ─── Structure & Breadcrumb ─────────────────────────────────────────────────

test.describe('Analytics Page — Structure', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
  });

  test('displays Analytics heading', async ({ page }) => {
    await expect(page.getByTestId('analytics-heading')).toBeVisible();
    await expect(page.getByTestId('analytics-heading')).toHaveText('Analytics');
  });

  test('displays breadcrumb with Overview, index name, and Analytics', async ({ page }) => {
    const breadcrumb = page.getByTestId('analytics-breadcrumb');
    await expect(breadcrumb).toBeVisible();
    await expect(breadcrumb.getByText('Overview')).toBeVisible();
    await expect(breadcrumb.getByText('test-index')).toBeVisible();
    await expect(breadcrumb.getByText('Analytics')).toBeVisible();
  });

  test('shows all 6 tab triggers', async ({ page }) => {
    await expect(page.getByTestId('tab-overview')).toBeVisible();
    await expect(page.getByTestId('tab-searches')).toBeVisible();
    await expect(page.getByTestId('tab-no-results')).toBeVisible();
    await expect(page.getByTestId('tab-filters')).toBeVisible();
    await expect(page.getByTestId('tab-devices')).toBeVisible();
    await expect(page.getByTestId('tab-geography')).toBeVisible();
  });

  test('shows date range buttons with 7d selected by default', async ({ page }) => {
    const rangeGroup = page.getByTestId('analytics-date-range');
    await expect(rangeGroup).toBeVisible();
    await expect(page.getByTestId('range-7d')).toHaveClass(/bg-primary/);
    await expect(page.getByTestId('range-30d')).not.toHaveClass(/bg-primary/);
    await expect(page.getByTestId('range-90d')).not.toHaveClass(/bg-primary/);
  });

  test('shows date range label', async ({ page }) => {
    await expect(page.getByTestId('analytics-date-label')).toBeVisible();
  });

  test('shows Update button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /update/i })).toBeVisible();
  });

  test('shows Clear Analytics button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /clear analytics/i })).toBeVisible();
  });
});

// ─── Date Range Switching ───────────────────────────────────────────────────

test.describe('Analytics Page — Date Range', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
  });

  test('switching to 30d highlights 30d and unhighlights 7d', async ({ page }) => {
    await page.getByTestId('range-30d').click();
    await expect(page.getByTestId('range-30d')).toHaveClass(/bg-primary/);
    await expect(page.getByTestId('range-7d')).not.toHaveClass(/bg-primary/);
  });

  test('switching to 90d highlights 90d', async ({ page }) => {
    await page.getByTestId('range-90d').click();
    await expect(page.getByTestId('range-90d')).toHaveClass(/bg-primary/);
    await expect(page.getByTestId('range-7d')).not.toHaveClass(/bg-primary/);
  });
});

// ─── Overview Tab (default) ─────────────────────────────────────────────────

test.describe('Analytics Page — Overview Tab', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
  });

  test('shows KPI cards container', async ({ page }) => {
    await expect(page.getByTestId('kpi-cards')).toBeVisible();
  });

  test('Total Searches KPI shows "15,420"', async ({ page }) => {
    const card = page.getByTestId('kpi-total-searches');
    await expect(card).toBeVisible();
    await expect(card).toContainText('15,420');
  });

  test('Unique Users KPI shows "3,280"', async ({ page }) => {
    const card = page.getByTestId('kpi-unique-users');
    await expect(card).toBeVisible();
    await expect(card).toContainText('3,280');
  });

  test('No-Result Rate KPI shows "4.5%"', async ({ page }) => {
    const card = page.getByTestId('kpi-no-result-rate');
    await expect(card).toBeVisible();
    await expect(card).toContainText('4.5%');
  });

  test('shows Search Volume chart card', async ({ page }) => {
    await expect(page.getByTestId('search-volume-chart')).toBeVisible();
    await expect(page.getByText('Search Volume')).toBeVisible();
  });

  test('shows No-Result Rate Over Time chart card', async ({ page }) => {
    await expect(page.getByTestId('no-result-rate-chart')).toBeVisible();
    await expect(page.getByText('No-Result Rate Over Time')).toBeVisible();
  });

  test('shows Top 10 Searches table with mock data', async ({ page }) => {
    const card = page.getByTestId('top-searches-overview');
    await expect(card).toBeVisible();
    await expect(card.getByText('Top 10 Searches')).toBeVisible();
    await expect(card.getByText('wireless headphones')).toBeVisible();
    await expect(card.getByText('laptop stand')).toBeVisible();
    await expect(card.getByText('1,520')).toBeVisible();
  });
});

// ─── Searches Tab ───────────────────────────────────────────────────────────

test.describe('Analytics Page — Searches Tab', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-searches').click();
  });

  test('shows filter input', async ({ page }) => {
    await expect(page.getByTestId('searches-filter-input')).toBeVisible();
  });

  test('shows country filter dropdown with mock countries', async ({ page }) => {
    const dropdown = page.getByTestId('searches-country-filter');
    await expect(dropdown).toBeVisible();
    await expect(dropdown).toContainText('All Countries');
  });

  test('shows device filter dropdown with mock platforms', async ({ page }) => {
    const dropdown = page.getByTestId('searches-device-filter');
    await expect(dropdown).toBeVisible();
    await expect(dropdown).toContainText('All Devices');
  });

  test('shows Top Searches table with query data', async ({ page }) => {
    const table = page.getByTestId('top-searches-table');
    await expect(table).toBeVisible();
    await expect(table.getByText('wireless headphones')).toBeVisible();
    await expect(table.getByText('usb-c cable')).toBeVisible();
    await expect(table.getByText('mechanical keyboard')).toBeVisible();
  });

  test('shows query count "5 queries"', async ({ page }) => {
    await expect(page.getByText('5 queries')).toBeVisible();
  });

  test('shows avg hits column with values', async ({ page }) => {
    const table = page.getByTestId('top-searches-table');
    await expect(table.getByText('Avg Hits')).toBeVisible();
    // wireless headphones has nbHits: 45
    await expect(table.getByText('45')).toBeVisible();
  });
});

// ─── No Results Tab ─────────────────────────────────────────────────────────

test.describe('Analytics Page — No Results Tab', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-no-results').click();
  });

  test('shows no-result rate banner with "4.5%"', async ({ page }) => {
    const banner = page.getByTestId('no-result-rate-banner');
    await expect(banner).toBeVisible();
    await expect(banner).toContainText('4.5%');
  });

  test('banner shows "of searches return no results" message', async ({ page }) => {
    const banner = page.getByTestId('no-result-rate-banner');
    await expect(banner).toContainText('of searches return no results');
  });

  test('shows no-results table with zero-result queries', async ({ page }) => {
    const table = page.getByTestId('no-results-table');
    await expect(table).toBeVisible();
    await expect(table.getByText('unicorn widget')).toBeVisible();
    await expect(table.getByText('nonexistent product')).toBeVisible();
    await expect(table.getByText('xyzzy123')).toBeVisible();
  });

  test('shows query counts in table', async ({ page }) => {
    const table = page.getByTestId('no-results-table');
    await expect(table.getByText('42')).toBeVisible();
    await expect(table.getByText('18')).toBeVisible();
  });
});

// ─── Filters Tab ────────────────────────────────────────────────────────────

test.describe('Analytics Page — Filters Tab', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-filters').click();
  });

  test('shows filters table with filter attributes', async ({ page }) => {
    const table = page.getByTestId('filters-table');
    await expect(table).toBeVisible();
    await expect(table.getByText('brand:Apple')).toBeVisible();
    await expect(table.getByText('brand:Samsung')).toBeVisible();
    await expect(table.getByText('category:Electronics')).toBeVisible();
  });

  test('shows filter counts', async ({ page }) => {
    const table = page.getByTestId('filters-table');
    await expect(table.getByText('3,200')).toBeVisible();
    await expect(table.getByText('5,100')).toBeVisible();
  });

  test('shows "Filters Causing No Results" section', async ({ page }) => {
    await expect(page.getByText('Filters Causing No Results')).toBeVisible();
    await expect(page.getByText('brand:Obsolete')).toBeVisible();
  });
});

// ─── Devices Tab ────────────────────────────────────────────────────────────

test.describe('Analytics Page — Devices Tab', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-devices').click();
  });

  test('shows Desktop card with count and percentage', async ({ page }) => {
    const card = page.getByTestId('device-desktop');
    await expect(card).toBeVisible();
    await expect(card).toContainText('Desktop');
    await expect(card).toContainText('9,200');
    await expect(card).toContainText('59.7%');
  });

  test('shows Mobile card with count and percentage', async ({ page }) => {
    const card = page.getByTestId('device-mobile');
    await expect(card).toBeVisible();
    await expect(card).toContainText('Mobile');
    await expect(card).toContainText('4,600');
    await expect(card).toContainText('29.8%');
  });

  test('shows Tablet card with count and percentage', async ({ page }) => {
    const card = page.getByTestId('device-tablet');
    await expect(card).toBeVisible();
    await expect(card).toContainText('Tablet');
    await expect(card).toContainText('1,620');
    await expect(card).toContainText('10.5%');
  });

  test('shows stacked area chart with SVG content', async ({ page }) => {
    await expect(page.getByText('Searches by Device Over Time')).toBeVisible();
    // Verify the Recharts AreaChart actually rendered (not just the heading)
    const chartCard = page.getByText('Searches by Device Over Time').locator('xpath=ancestor::div[contains(@class,"rounded-lg")]');
    await expect(chartCard.locator('svg').first()).toBeAttached();
  });
});

// ─── Geography Tab ──────────────────────────────────────────────────────────

test.describe('Analytics Page — Geography Tab', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-geography').click();
  });

  test('shows countries count summary card', async ({ page }) => {
    const card = page.getByTestId('geo-countries-count');
    await expect(card).toBeVisible();
    await expect(card).toContainText('Countries');
    // 3 countries in mock data
    await expect(card).toContainText('3');
  });

  test('shows total searches summary card', async ({ page }) => {
    await expect(page.getByText('Total Searches')).toBeVisible();
    await expect(page.getByText('15,420')).toBeVisible();
  });

  test('shows country table with United States, United Kingdom, Germany', async ({ page }) => {
    await expect(page.getByText('United States')).toBeVisible();
    await expect(page.getByText('United Kingdom')).toBeVisible();
    await expect(page.getByText('Germany')).toBeVisible();
  });

  test('shows search counts and percentages for countries', async ({ page }) => {
    await expect(page.getByText('8,500')).toBeVisible();
    await expect(page.getByText('55.1%')).toBeVisible();
  });

  test('clicking a country shows drill-down with top searches and regions', async ({ page }) => {
    // Click United States row
    await page.getByText('United States').click();

    // Should show drill-down view
    await expect(page.getByRole('button', { name: /all countries/i })).toBeVisible();
    await expect(page.getByText('Top Searches from United States')).toBeVisible();

    // Top searches from US
    await expect(page.getByText('wireless headphones')).toBeVisible();
    await expect(page.getByText('520')).toBeVisible();

    // States breakdown
    await expect(page.getByRole('heading', { name: 'States', exact: true })).toBeVisible();
    await expect(page.getByText('California')).toBeVisible();
    await expect(page.getByText('New York')).toBeVisible();
    await expect(page.getByText('Texas')).toBeVisible();
  });

  test('clicking "All Countries" in drill-down returns to country list', async ({ page }) => {
    // Go to drill-down
    await page.getByText('United States').click();
    await expect(page.getByText('Top Searches from United States')).toBeVisible();

    // Go back
    await page.getByRole('button', { name: /all countries/i }).click();
    await expect(page.getByText('Searches by Country')).toBeVisible();
    await expect(page.getByText('United States')).toBeVisible();
  });
});

// ─── Actions ────────────────────────────────────────────────────────────────

test.describe('Analytics Page — Actions', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
  });

  test('clicking Update sends POST to /2/analytics/flush', async ({ page }) => {
    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/2/analytics/flush') &&
        response.request().method() === 'POST' &&
        response.status() === 200,
    );
    await page.getByRole('button', { name: /update/i }).click();
    await responsePromise;
  });

  test('clicking Clear Analytics shows confirm dialog and sends DELETE', async ({ page }) => {
    // Accept the confirm dialog
    let confirmMessage = '';
    page.on('dialog', async (dialog) => {
      confirmMessage = dialog.message();
      await dialog.accept();
    });

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/2/analytics/clear') &&
        response.request().method() === 'DELETE' &&
        response.status() === 200,
    );

    await page.getByRole('button', { name: /clear analytics/i }).click();
    await responsePromise;

    expect(confirmMessage).toContain('test-index');
  });
});

// ─── Navigation ─────────────────────────────────────────────────────────────

test.describe('Analytics Page — Navigation', () => {
  test('clicking index breadcrumb navigates back to index page', async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');

    const breadcrumb = page.getByTestId('analytics-breadcrumb');
    await breadcrumb.getByText('test-index').click();
    await expect(page).toHaveURL(/\/index\/test-index$/);
  });
});

// ─── Empty States ────────────────────────────────────────────────────────────

test.describe('Analytics Page — Empty States', () => {
  async function mockEmptyAnalytics(page: Page) {
    await page.route((reqUrl) => reqUrl.pathname.startsWith('/2/'), (route) => {
      const pathname = new URL(route.request().url()).pathname;
      const method = route.request().method();
      if (method === 'POST' && pathname === '/2/analytics/flush') {
        return route.fulfill({ status: 200, contentType: 'application/json', body: '{"message":"ok"}' });
      }
      if (method === 'DELETE' && pathname === '/2/analytics/clear') {
        return route.fulfill({ status: 200, contentType: 'application/json', body: '{"message":"ok"}' });
      }

      let body: any = {};
      if (pathname === '/2/searches/count') body = { count: 0, dates: [] };
      else if (pathname === '/2/searches/noResultRate') body = { rate: 0, dates: [] };
      else if (pathname === '/2/searches/noResults') body = { searches: [] };
      else if (pathname === '/2/searches') body = { searches: [] };
      else if (pathname === '/2/users/count') body = { count: 0, dates: [] };
      else if (pathname === '/2/filters/noResults') body = { filters: [] };
      else if (pathname === '/2/filters') body = { filters: [] };
      else if (pathname === '/2/devices') body = { platforms: [], dates: [] };
      else if (pathname === '/2/geo') body = { countries: [], total: 0 };
      else if (pathname === '/2/overview') body = {};
      else if (pathname === '/2/status') body = { enabled: true };

      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    });
    await mockIndicesApi(page);
  }

  test('Overview tab shows "No search data yet" empty state for chart', async ({ page }) => {
    await mockEmptyAnalytics(page);
    await page.goto('/index/test-index/analytics');
    await expect(page.getByTestId('empty-state')).toBeVisible();
    await expect(page.getByRole('heading', { name: 'No search data yet' })).toBeVisible();
  });

  test('Searches tab shows empty state with no queries', async ({ page }) => {
    await mockEmptyAnalytics(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-searches').click();
    await expect(page.getByText('No searches recorded yet')).toBeVisible();
  });

  test('Filters tab shows "No filter usage recorded" empty state', async ({ page }) => {
    await mockEmptyAnalytics(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-filters').click();
    await expect(page.getByText('No filter usage recorded')).toBeVisible();
  });

  test('Devices tab shows "No device data" empty state', async ({ page }) => {
    await mockEmptyAnalytics(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-devices').click();
    await expect(page.getByText('No device data')).toBeVisible();
  });

  test('Geography tab shows "No geographic data" empty state', async ({ page }) => {
    await mockEmptyAnalytics(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-geography').click();
    await expect(page.getByText('No geographic data')).toBeVisible();
  });
});

// ─── Error States ────────────────────────────────────────────────────────────

test.describe('Analytics Page — Error States', () => {
  test('Searches tab shows error state when API fails', async ({ page }) => {
    await page.route((reqUrl) => reqUrl.pathname.startsWith('/2/'), (route) => {
      const pathname = new URL(route.request().url()).pathname;
      if (pathname === '/2/searches') {
        return route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"Internal Server Error"}' });
      }
      // Other endpoints return empty but valid data
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({}) });
    });
    await mockIndicesApi(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-searches').click();
    await expect(page.getByTestId('error-state')).toBeVisible();
  });

  test('Filters tab shows error state when API fails', async ({ page }) => {
    await page.route((reqUrl) => reqUrl.pathname.startsWith('/2/'), (route) => {
      const pathname = new URL(route.request().url()).pathname;
      if (pathname === '/2/filters') {
        return route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"Internal Server Error"}' });
      }
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({}) });
    });
    await mockIndicesApi(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-filters').click();
    await expect(page.getByTestId('error-state')).toBeVisible();
  });
});

// ─── DeltaBadge / Trend Indicators ──────────────────────────────────────────

test.describe('Analytics Page — DeltaBadge Indicators', () => {
  test('shows delta badge with percentage change on KPI cards', async ({ page }) => {
    // Mock current period with higher values than previous period
    await page.route((reqUrl) => reqUrl.pathname.startsWith('/2/'), (route) => {
      const pathname = new URL(route.request().url()).pathname;
      const url = route.request().url();

      if (pathname === '/2/searches/count') {
        // Check if this is the "previous" range query (has earlier dates)
        if (url.includes('startDate') && url.includes('endDate')) {
          const startMatch = url.match(/startDate=([^&]*)/);
          if (startMatch) {
            const startDate = startMatch[1];
            // Previous period has earlier dates — return lower count
            if (startDate < '2026-01-25') {
              return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ count: 10000, dates: [] }) });
            }
          }
        }
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_SEARCH_COUNT) });
      }
      if (pathname === '/2/users/count') {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_USERS_COUNT) });
      }
      if (pathname === '/2/searches/noResultRate') {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_NO_RESULT_RATE) });
      }
      if (pathname === '/2/searches') {
        return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(MOCK_TOP_SEARCHES) });
      }

      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({}) });
    });
    await mockIndicesApi(page);
    await page.goto('/index/test-index/analytics');

    // Wait for KPI cards to load
    await expect(page.getByTestId('kpi-total-searches')).toContainText('15,420');

    // Delta badges should be present on KPI cards (data-testid="delta-badge")
    const badges = page.locator('[data-testid="delta-badge"]');
    await expect(badges.first()).toBeVisible();
  });
});

// ─── Searches Tab — Sorting ─────────────────────────────────────────────────

test.describe('Analytics Page — Searches Tab Sorting', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-searches').click();
  });

  test('clicking Count column header toggles sort direction', async ({ page }) => {
    const table = page.getByTestId('top-searches-table');
    await expect(table.getByText('wireless headphones')).toBeVisible();

    // Count is the default sort column (desc). Click to toggle to asc.
    // The header is a <th> element
    await table.locator('th').filter({ hasText: 'Count' }).click();

    // The sort arrow should flip — the first row should now be the lowest count
    // "monitor arm" has count 430 (lowest)
    const firstRow = table.locator('tbody tr').first();
    await expect(firstRow.getByText('monitor arm')).toBeVisible();
  });

  test('clicking Query column header sorts alphabetically', async ({ page }) => {
    const table = page.getByTestId('top-searches-table');
    await table.locator('th').filter({ hasText: 'Query' }).click();

    // Sort indicator should appear on Query column
    await expect(table.getByText('↓').or(table.getByText('↑'))).toBeVisible();
  });
});

// ─── Searches Tab — Text Filter ─────────────────────────────────────────────

test.describe('Analytics Page — Searches Tab Filter', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-searches').click();
  });

  test('typing in filter input narrows displayed queries', async ({ page }) => {
    const filterInput = page.getByTestId('searches-filter-input');
    await expect(filterInput).toBeVisible();

    // All 5 queries visible initially
    const table = page.getByTestId('top-searches-table');
    await expect(table.getByText('wireless headphones')).toBeVisible();
    await expect(table.getByText('usb-c cable')).toBeVisible();

    // Filter to only show "wireless"
    await filterInput.fill('wireless');

    // Only matching query should remain
    await expect(table.getByText('wireless headphones')).toBeVisible();
    await expect(table.getByText('usb-c cable')).not.toBeVisible();
    await expect(table.getByText('mechanical keyboard')).not.toBeVisible();
  });

  test('filter shows updated query count', async ({ page }) => {
    const filterInput = page.getByTestId('searches-filter-input');
    await filterInput.fill('keyboard');

    // Only 1 result should match
    await expect(page.getByText('1 queries')).toBeVisible();
  });
});

// ─── Filters Tab — Expand/Collapse ──────────────────────────────────────────

test.describe('Analytics Page — Filters Tab Expand', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');
    await page.getByTestId('tab-filters').click();
  });

  test('clicking a filter row expands to show filter values', async ({ page }) => {
    const table = page.getByTestId('filters-table');
    await expect(table.getByText('brand:Apple')).toBeVisible();

    // Click to expand
    await table.getByText('brand:Apple').click();

    // Should show child values loaded from /2/filters/{attribute}
    await expect(page.getByText('SomeValue').first()).toBeVisible();
  });

  test('clicking same row again collapses values', async ({ page }) => {
    const table = page.getByTestId('filters-table');

    // Expand
    await table.getByText('brand:Apple').click();
    await expect(page.getByText('SomeValue').first()).toBeVisible();

    // Collapse
    await table.getByText('brand:Apple').click();
    await expect(page.getByText('SomeValue').first()).not.toBeVisible();
  });
});

// ─── Country / Device Filters ────────────────────────────────────────────────

test.describe('Analytics Page — Country Filter', () => {
  test('selecting a country from dropdown triggers new API call with country param', async ({ page }) => {
    const searchRequests: string[] = [];

    await setupPage(page);

    // Track /2/searches requests via event listener (doesn't conflict with route handlers)
    page.on('request', (req) => {
      if (new URL(req.url()).pathname === '/2/searches') {
        searchRequests.push(req.url());
      }
    });

    await page.goto('/index/test-index/analytics');

    // Navigate to Searches tab
    await page.getByTestId('tab-searches').click();
    await expect(page.getByTestId('searches-country-filter')).toBeVisible();

    const initialCount = searchRequests.length;

    // Select "US" from the country dropdown
    await page.getByTestId('searches-country-filter').selectOption('US');

    // Wait for new API call
    await expect.poll(() => searchRequests.length).toBeGreaterThan(initialCount);

    // The new request should include country parameter
    const latestRequest = searchRequests[searchRequests.length - 1];
    expect(latestRequest).toContain('country=US');
  });
});

test.describe('Analytics Page — Device Filter', () => {
  test('selecting a device from dropdown triggers new API call with tags param', async ({ page }) => {
    const searchRequests: string[] = [];

    await setupPage(page);

    // Track /2/searches requests via event listener (doesn't conflict with route handlers)
    page.on('request', (req) => {
      if (new URL(req.url()).pathname === '/2/searches') {
        searchRequests.push(req.url());
      }
    });

    await page.goto('/index/test-index/analytics');

    // Navigate to Searches tab
    await page.getByTestId('tab-searches').click();
    await expect(page.getByTestId('searches-device-filter')).toBeVisible();

    const initialCount = searchRequests.length;

    // Select "mobile" from device dropdown
    await page.getByTestId('searches-device-filter').selectOption('mobile');

    // Wait for new API call
    await expect.poll(() => searchRequests.length).toBeGreaterThan(initialCount);

    // The new request should include tags parameter for platform
    const latestRequest = searchRequests[searchRequests.length - 1];
    expect(latestRequest).toContain('tags=platform');
  });
});

// ─── Clear Analytics — Cancel ───────────────────────────────────────────────

test.describe('Analytics Page — Clear Analytics Cancel', () => {
  test('dismissing confirm dialog does NOT send DELETE request', async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');

    let deleteRequestFired = false;
    page.on('request', (request) => {
      if (request.url().includes('/2/analytics/clear') && request.method() === 'DELETE') {
        deleteRequestFired = true;
      }
    });

    // Dismiss the confirm dialog
    page.on('dialog', async (dialog) => {
      await dialog.dismiss();
    });

    await page.getByRole('button', { name: /clear analytics/i }).click();

    // After dialog dismiss, verify the button is still visible (UI settled) and no DELETE fired
    await expect(page.getByRole('button', { name: /clear analytics/i })).toBeVisible();
    expect(deleteRequestFired).toBe(false);
  });
});

// ─── Sparklines ─────────────────────────────────────────────────────────────

test.describe('Analytics Page — Sparklines', () => {
  test('KPI cards with sparkData render sparkline SVGs', async ({ page }) => {
    await setupPage(page);
    await page.goto('/index/test-index/analytics');

    // Total Searches and No-Result Rate KPIs have sparkData in mocks
    // They should render a sparkline element
    const sparklines = page.locator('[data-testid="sparkline"]');
    await expect(sparklines.first()).toBeVisible();

    // Sparklines contain Recharts SVG
    const svg = sparklines.first().locator('svg');
    await expect(svg).toBeAttached();
  });
});
