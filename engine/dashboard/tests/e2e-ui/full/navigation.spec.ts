/**
 * E2E-UI Full Suite — Navigation, Layout & Header (Real Server)
 *
 * Tests the sidebar navigation, header controls, cross-page navigation,
 * theme toggle, connection status, and layout behavior.
 *
 * NO mocking. Tests verify real navigation between pages and UI state.
 *
 * Pre-requisites:
 *   - Flapjack server running on the repo-local configured backend port
 *   - `e2e-products` index seeded with 12 products (via seed.setup.ts)
 *   - Vite dev server on the repo-local configured dashboard port
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { TEST_INDEX } from '../helpers';

test.describe('Navigation & Layout', () => {

  // =========================================================================
  // Sidebar Navigation
  // =========================================================================

  test('sidebar shows all main navigation items', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));

    // All top-level nav links should be present
    await expect(sidebar.getByText('Overview').first()).toBeVisible();
    await expect(sidebar.getByText('API Logs').first()).toBeVisible();
    await expect(sidebar.getByText('Migrate').first()).toBeVisible();
    await expect(sidebar.getByText('API Keys').first()).toBeVisible();
    await expect(sidebar.getByText('Metrics').first()).toBeVisible();
    await expect(sidebar.getByText('System').first()).toBeVisible();
  });

  test('sidebar shows seeded index in indexes section', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    // The sidebar should list e2e-products under INDEXES section
    const sidebar = page.locator('aside').or(page.locator('nav'));
    await expect(sidebar.getByText(TEST_INDEX).first()).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar Overview navigates to overview page', async ({ page }) => {
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(page.getByTestId('results-panel').or(page.getByText(/no results found/i))).toBeVisible({ timeout: 15_000 });

    // Click Overview in sidebar
    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('Overview').first().click();
    await expect(page).toHaveURL(/\/overview/);
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar API Logs navigates to logs page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('API Logs').first().click();
    await expect(page).toHaveURL(/\/logs/);
    await expect(page.getByRole('heading', { name: /api log/i })).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar Migrate navigates to migrate page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('Migrate').first().click();
    await expect(page).toHaveURL(/\/migrate/);
    await expect(page.getByRole('heading', { name: /migrate/i })).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar API Keys navigates to keys page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('API Keys').first().click();
    await expect(page).toHaveURL(/\/keys/);
    await expect(page.getByRole('heading', { name: 'API Keys', exact: true })).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar Metrics navigates to metrics page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('Metrics').first().click();
    await expect(page).toHaveURL(/\/metrics/);
    await expect(page.getByRole('heading', { name: /metrics/i })).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar System navigates to system page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText('System').first().click();
    await expect(page).toHaveURL(/\/system/);
    await expect(page.getByRole('heading', { name: /system/i })).toBeVisible({ timeout: 10_000 });
  });

  test('clicking sidebar index link navigates to search page', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    const sidebar = page.locator('aside').or(page.locator('nav'));
    await sidebar.getByText(TEST_INDEX).first().click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}`));
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });
  });

  // =========================================================================
  // Header
  // =========================================================================

  test('header shows Flapjack logo and connection status', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    // Flapjack logo/brand text in header
    await expect(page.getByText('Flapjack').first()).toBeVisible();

    // Connection status badge — should show "Connected" since server is running and auth is seeded
    await expect(page.getByText('Connected')).toBeVisible({ timeout: 10_000 });
  });

  test('theme toggle switches between light and dark mode', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    // Click the theme toggle button
    const themeBtn = page.getByRole('button', { name: /toggle theme/i });
    await expect(themeBtn).toBeVisible();

    // Get initial theme state
    const htmlBefore = await page.locator('html').getAttribute('class');

    // Click to toggle — wait for the class to actually change (Playwright retries automatically)
    await themeBtn.click();
    await expect(page.locator('html')).not.toHaveAttribute('class', htmlBefore ?? '');

    // Theme class should change
    const htmlAfter = await page.locator('html').getAttribute('class');
    expect(htmlBefore).not.toBe(htmlAfter);

    // Toggle back — wait for class to return to original
    await themeBtn.click();
    await expect(page.locator('html')).toHaveAttribute('class', htmlBefore ?? '');
    const htmlRestored = await page.locator('html').getAttribute('class');
    expect(htmlRestored).toBe(htmlBefore);
  });

  test('indexing queue button opens empty queue panel', async ({ page }) => {
    await page.goto('/overview');
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });

    // Click the queue button
    const queueBtn = page.getByRole('button', { name: /indexing queue/i });
    await expect(queueBtn).toBeVisible();
    await queueBtn.click();

    // Queue panel should appear and show idle state
    await expect(page.getByText('Indexing Queue')).toBeVisible({ timeout: 5_000 });
    await expect(page.getByText(/no active tasks|all clear/i).first()).toBeVisible();
  });

  // =========================================================================
  // Cross-page Navigation (Search → sub-pages)
  // =========================================================================

  test('search page nav buttons lead to correct sub-pages', async ({ page }) => {
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Click Synonyms nav button
    await page.getByRole('link', { name: /synonyms/i }).click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}/synonyms`));
    await expect(page.getByText('Synonyms').first()).toBeVisible({ timeout: 10_000 });

    // Navigate back to search
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Click Analytics nav button
    await page.getByRole('link', { name: /analytics/i }).click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}/analytics`));
    await expect(page.getByTestId('analytics-heading')).toBeVisible({ timeout: 15_000 });

    // Navigate back to search
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Click Settings nav button
    await page.getByRole('link', { name: /settings/i }).click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}/settings`));
    await expect(page.getByRole('heading', { name: /settings/i })).toBeVisible({ timeout: 10_000 });

    // Navigate back to search
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Click Merchandising nav button
    await page.getByRole('link', { name: /merchandising/i }).click();
    await expect(page).toHaveURL(new RegExp(`/index/${TEST_INDEX}/merchandising`));
    await expect(page.getByText('Merchandising Studio').first()).toBeVisible({ timeout: 15_000 });
  });

  test('search page breadcrumb navigates back to overview', async ({ page }) => {
    await page.goto(`/index/${TEST_INDEX}`);
    await expect(
      page.getByTestId('results-panel').or(page.getByText(/no results found/i))
    ).toBeVisible({ timeout: 15_000 });

    // Click the Overview breadcrumb link
    await page.getByRole('link', { name: /overview/i }).first().click();
    await expect(page).toHaveURL(/\/overview/);
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 10_000 });
  });

  // =========================================================================
  // 404 / Not Found
  // =========================================================================

  test('navigating to unknown route shows page not found', async ({ page }) => {
    await page.goto('/nonexistent-page-12345');
    await expect(page.getByText('Page not found')).toBeVisible({ timeout: 10_000 });
  });
});
