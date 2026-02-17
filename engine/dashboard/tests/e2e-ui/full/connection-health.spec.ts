/**
 * E2E-UI Full Suite -- Connection Health & Disconnected State (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests that the dashboard correctly detects and displays connection status.
 *
 * Covers:
 * - Shows "Connected" badge when server is healthy
 * - BETA badge is always visible in header
 * - Disconnected banner appears when health check fails
 * - Connection status recovers when server becomes available again
 */
import { test, expect } from '../../fixtures/auth.fixture';

test.describe('Connection Health', () => {
  test('shows Connected badge when server is healthy', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByText('Overview').first()).toBeVisible({ timeout: 15_000 });

    // Should show Connected status
    await expect(page.getByText('Connected').first()).toBeVisible({ timeout: 10_000 });

    // No disconnected banner should be visible
    await expect(page.locator('[data-testid="disconnected-banner"]')).not.toBeVisible();
  });

  test('BETA badge is always visible in header', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByText('Overview').first()).toBeVisible({ timeout: 15_000 });

    // BETA badge should always be visible
    await expect(page.getByText('Beta').first()).toBeVisible();
  });

  test('shows disconnected banner when server is unreachable', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByText('Overview').first()).toBeVisible({ timeout: 15_000 });

    // Block /health requests to simulate server going down
    await page.route('**/health', (route) => route.abort('connectionrefused'));

    // Wait for the health check to fail and banner to appear (3s interval + retry)
    await expect(page.locator('[data-testid="disconnected-banner"]')).toBeVisible({ timeout: 15_000 });

    // Should show disconnected status
    await expect(page.getByText('Disconnected').first()).toBeVisible();

    // Banner should mention the server
    await expect(page.locator('[data-testid="disconnected-banner"]')).toContainText('Server disconnected');
  });

  test('recovers from disconnected state when server comes back', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByText('Overview').first()).toBeVisible({ timeout: 15_000 });

    // Block health to simulate disconnect
    await page.route('**/health', (route) => route.abort('connectionrefused'));
    await expect(page.locator('[data-testid="disconnected-banner"]')).toBeVisible({ timeout: 15_000 });

    // Unblock health to simulate recovery
    await page.unrouteAll({ behavior: 'ignoreErrors' });

    // Should recover — banner should disappear
    await expect(page.locator('[data-testid="disconnected-banner"]')).not.toBeVisible({ timeout: 15_000 });
    await expect(page.getByText('Connected').first()).toBeVisible({ timeout: 10_000 });
  });
});
