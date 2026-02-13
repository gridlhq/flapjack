import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

const MOCK_INDICES_SMALL = {
  items: [
    { uid: 'products', name: 'products', createdAt: '2024-01-01T00:00:00Z', updatedAt: '2024-01-01T00:00:00Z', entries: 100, dataSize: 5000, numberOfPendingTasks: 0 },
    { uid: 'users', name: 'users', createdAt: '2024-01-01T00:00:00Z', updatedAt: '2024-01-01T00:00:00Z', entries: 50, dataSize: 2000, numberOfPendingTasks: 0 },
  ],
  nbPages: 1,
};

const MOCK_INDICES_MANY = {
  items: Array.from({ length: 8 }, (_, i) => ({
    uid: `index-${i}`,
    name: `index-${i}`,
    createdAt: '2024-01-01T00:00:00Z',
    updatedAt: '2024-01-01T00:00:00Z',
    entries: 10 * (i + 1),
    dataSize: 1000 * (i + 1),
    numberOfPendingTasks: 0,
  })),
  nbPages: 1,
};

function mockIndicesApi(page: Page, response: typeof MOCK_INDICES_SMALL) {
  return page.route('**/1/indexes', (route) => {
    if (route.request().method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(response),
      });
    }
    return route.fallback();
  });
}

function mockEmptyIndices(page: Page) {
  return page.route('**/1/indexes', (route) => {
    if (route.request().method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ items: [], nbPages: 0 }),
      });
    }
    return route.fallback();
  });
}

function mockHealthApi(page: Page) {
  return page.route('**/health', (route) => {
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ status: 'ok' }) });
  });
}

test.describe('Navigation & Layout', () => {
  test.beforeEach(async ({ page }) => {
    await mockEmptyIndices(page);
    await mockHealthApi(page);
    await page.goto('/');
  });

  test('should display header with title', async ({ page }) => {
    const header = page.locator('header');
    await expect(header).toBeVisible();

    await expect(page.getByText('Flapjack').first()).toBeVisible();
  });

  test('should display sidebar navigation', async ({ page }) => {
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible();

    // Check for all navigation links within the sidebar (Analytics is per-index, not in sidebar)
    await expect(sidebar.getByRole('link', { name: /overview/i })).toBeVisible();
    await expect(sidebar.getByRole('link', { name: /api keys/i })).toBeVisible();
    await expect(sidebar.getByRole('link', { name: /api logs/i })).toBeVisible();
    await expect(sidebar.getByRole('link', { name: /migrate/i })).toBeVisible();
    await expect(sidebar.getByRole('link', { name: /system/i })).toBeVisible();
  });

  test('should navigate between pages using sidebar', async ({ page }) => {
    const sidebar = page.locator('aside');

    // Navigate to API Keys
    await sidebar.getByRole('link', { name: /api keys/i }).click();
    await expect(page).toHaveURL(/\/keys/);

    // Navigate to Migrate
    await sidebar.getByRole('link', { name: /migrate/i }).click();
    await expect(page).toHaveURL(/\/migrate/);

    // Navigate back to Overview
    await sidebar.getByRole('link', { name: /overview/i }).click();
    await expect(page).toHaveURL(/\/overview/);
  });

  test('should highlight active navigation item', async ({ page }) => {
    // Go to API Keys page
    await page.goto('/keys');

    const keysLink = page.getByRole('link', { name: /api keys/i });
    // Active links get bg-primary/15 class
    await expect(keysLink).toHaveClass(/bg-primary\/15/);
  });

  test('should show connection status as Connected', async ({ page }) => {
    // Scope to the header area to avoid matching "Connected" text anywhere else on the page
    const header = page.locator('header');
    await expect(header.getByText('Connected')).toBeVisible();
  });

  test('should preserve dark mode preference across navigation', async ({ page }) => {
    const darkModeToggle = page.getByRole('button', { name: /toggle.*theme/i });
    await darkModeToggle.click();

    // Navigate to another page
    await page.getByRole('link', { name: /api keys/i }).click();

    const html = page.locator('html');
    await expect(html).toHaveClass(/dark/);
  });

  test('should handle 404 for unknown routes', async ({ page }) => {
    await page.goto('/this-route-does-not-exist');

    await expect(page.getByText(/page not found/i)).toBeVisible();
  });

  test('should navigate to home when clicking Flapjack logo', async ({ page }) => {
    // Navigate away from home first
    await page.getByRole('link', { name: /api keys/i }).click();
    await expect(page).toHaveURL(/\/keys/);

    // Click the Flapjack logo link in the header
    await page.getByRole('link', { name: /flapjack/i }).click();
    await expect(page).toHaveURL(/\/$/);
  });

  test('should open connection settings dialog', async ({ page }) => {
    const settingsButton = page.getByRole('button', { name: /connection settings/i });
    await settingsButton.click();

    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(page.getByText(/admin api key/i)).toBeVisible();
  });
});

test.describe('Navigation — Header Features', () => {
  test.beforeEach(async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_SMALL);
    await mockHealthApi(page);
    await page.goto('/');
  });

  test('shows "API Docs" link in header', async ({ page }) => {
    const header = page.locator('header');
    const apiDocsLink = header.getByRole('link', { name: /api docs/i });
    await expect(apiDocsLink).toBeVisible();
    await expect(apiDocsLink).toHaveAttribute('href', '/swagger-ui');
    await expect(apiDocsLink).toHaveAttribute('target', '_blank');
  });

  test('shows indexing queue button in header', async ({ page }) => {
    const header = page.locator('header');
    const queueBtn = header.getByRole('button', { name: /indexing queue/i });
    await expect(queueBtn).toBeVisible();
  });

  test('clicking queue button opens queue panel showing "All clear"', async ({ page }) => {
    const header = page.locator('header');
    await header.getByRole('button', { name: /indexing queue/i }).click();

    // Panel should appear
    await expect(page.getByText('Indexing Queue')).toBeVisible();
    await expect(page.getByText('All clear')).toBeVisible();
  });

  test('clicking queue button again closes the panel', async ({ page }) => {
    const header = page.locator('header');
    const queueBtn = header.getByRole('button', { name: /indexing queue/i });

    await queueBtn.click();
    await expect(page.getByText('Indexing Queue')).toBeVisible();

    await queueBtn.click();
    await expect(page.getByText('Indexing Queue')).not.toBeVisible();
  });
});

test.describe('Navigation — Mobile Sidebar', () => {
  test('mobile menu toggle button is visible on small viewport', async ({ page }) => {
    await mockEmptyIndices(page);
    await mockHealthApi(page);

    // Set a mobile viewport
    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto('/');

    // The menu toggle button should be visible (md:hidden means shown on mobile)
    const menuToggle = page.getByRole('button', { name: /toggle navigation/i });
    await expect(menuToggle).toBeVisible();

    // Sidebar should be hidden by default on mobile
    const sidebar = page.locator('aside');
    await expect(sidebar).not.toBeVisible();
  });

  test('clicking mobile menu toggle opens sidebar overlay', async ({ page }) => {
    await mockEmptyIndices(page);
    await mockHealthApi(page);

    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto('/');

    // Open mobile sidebar
    await page.getByRole('button', { name: /toggle navigation/i }).click();

    // Sidebar should now be visible
    const sidebar = page.locator('aside');
    await expect(sidebar).toBeVisible();

    // Should show close button and overlay
    await expect(page.getByRole('button', { name: /close navigation/i })).toBeVisible();
  });

  test('clicking close button hides mobile sidebar', async ({ page }) => {
    await mockEmptyIndices(page);
    await mockHealthApi(page);

    await page.setViewportSize({ width: 375, height: 667 });
    await page.goto('/');

    // Open
    await page.getByRole('button', { name: /toggle navigation/i }).click();
    await expect(page.locator('aside')).toBeVisible();

    // Close
    await page.getByRole('button', { name: /close navigation/i }).click();
    await expect(page.locator('aside')).not.toBeVisible();
  });
});

test.describe('Sidebar — Indices Section', () => {
  test('should show indices section when indices exist', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_SMALL);
    await page.goto('/');

    const sidebar = page.locator('aside');
    const indicesSection = sidebar.getByTestId('sidebar-indices');
    await expect(indicesSection).toBeVisible();
    await expect(indicesSection.getByText('Indices')).toBeVisible();
  });

  test('should not show indices section when no indices exist', async ({ page }) => {
    await mockEmptyIndices(page);
    await page.goto('/');

    const sidebar = page.locator('aside');
    await expect(sidebar.getByTestId('sidebar-indices')).not.toBeVisible();
  });

  test('should show index links in sidebar', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_SMALL);
    await page.goto('/');

    const sidebar = page.locator('aside');
    await expect(sidebar.getByTestId('sidebar-index-products')).toBeVisible();
    await expect(sidebar.getByTestId('sidebar-index-users')).toBeVisible();
  });

  test('should navigate to index page when clicking index in sidebar', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_SMALL);
    await page.goto('/');

    const sidebar = page.locator('aside');
    await sidebar.getByTestId('sidebar-index-products').click();

    await expect(page).toHaveURL(/\/index\/products/);
  });

  test('should highlight active index in sidebar', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_SMALL);
    await page.goto('/index/products');

    const productsLink = page.getByTestId('sidebar-index-products');
    await expect(productsLink).toHaveClass(/bg-primary\/15/);

    // Other index should not be highlighted
    const usersLink = page.getByTestId('sidebar-index-users');
    await expect(usersLink).not.toHaveClass(/bg-primary\/15/);
  });

  test('should show "Show all" button when more than 5 indices', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_MANY);
    await page.goto('/');

    const showAllButton = page.getByTestId('sidebar-show-all-indices');
    await expect(showAllButton).toBeVisible();
    await expect(showAllButton).toHaveText(/show all \(8\)/i);
  });

  test('should not show "Show all" button when 5 or fewer indices', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_SMALL);
    await page.goto('/');

    await expect(page.getByTestId('sidebar-indices')).toBeVisible();
    await expect(page.getByTestId('sidebar-show-all-indices')).not.toBeVisible();
  });

  test('should expand to show all indices when clicking "Show all"', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_MANY);
    await page.goto('/');

    // Initially only 5 visible
    await expect(page.getByTestId('sidebar-index-index-0')).toBeVisible();
    await expect(page.getByTestId('sidebar-index-index-4')).toBeVisible();
    await expect(page.getByTestId('sidebar-index-index-5')).not.toBeVisible();

    // Click "Show all"
    await page.getByTestId('sidebar-show-all-indices').click();

    // All 8 indices should now be visible
    await expect(page.getByTestId('sidebar-index-index-7')).toBeVisible();

    // Button text should change to "Show less"
    await expect(page.getByTestId('sidebar-show-all-indices')).toHaveText(/show less/i);
  });

  test('should collapse indices when clicking "Show less"', async ({ page }) => {
    await mockIndicesApi(page, MOCK_INDICES_MANY);
    await page.goto('/');

    // Expand
    await page.getByTestId('sidebar-show-all-indices').click();
    await expect(page.getByTestId('sidebar-index-index-7')).toBeVisible();

    // Collapse
    await page.getByTestId('sidebar-show-all-indices').click();
    await expect(page.getByTestId('sidebar-index-index-5')).not.toBeVisible();
    await expect(page.getByTestId('sidebar-show-all-indices')).toHaveText(/show all \(8\)/i);
  });
});
