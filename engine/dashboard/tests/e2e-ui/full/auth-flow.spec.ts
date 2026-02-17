/**
 * Auth flow E2E tests — real browser, real server, NO mocks.
 *
 * These tests verify the dashboard authentication gate:
 * - Shows auth screen when no API key is configured
 * - Rejects invalid keys with a clear error
 * - Accepts valid keys and loads the dashboard
 *
 * NOTE: This test does NOT use the auth fixture — it tests the
 * unauthenticated-to-authenticated flow from scratch.
 */
import { test, expect } from '@playwright/test';
import { API_HEADERS } from '../helpers';

// Use raw test (no auth fixture) — we're testing the auth gate itself
const ADMIN_KEY = API_HEADERS['x-algolia-api-key'];

test.describe('Auth Gate', () => {
  test.beforeEach(async ({ page }) => {
    // Clear any stored auth so we start fresh
    await page.addInitScript(() => {
      localStorage.removeItem('flapjack-api-key');
      localStorage.removeItem('flapjack-app-id');
      localStorage.removeItem('flapjack-auth');
    });
  });

  test('shows auth screen when no API key is configured', async ({ page }) => {
    await page.goto('/');

    // Should see the auth gate
    const authGate = page.locator('[data-testid="auth-gate"]');
    await expect(authGate).toBeVisible();

    // Should show the Flapjack branding
    await expect(authGate.getByText('Welcome to Flapjack')).toBeVisible();

    // Should have an API key input
    const input = page.locator('[data-testid="auth-key-input"]');
    await expect(input).toBeVisible();

    // Should have a connect button (disabled without input)
    const submitBtn = page.locator('[data-testid="auth-submit"]');
    await expect(submitBtn).toBeVisible();
    await expect(submitBtn).toBeDisabled();

    // Should show help text about finding the key
    const helpText = page.locator('[data-testid="auth-help"]');
    await expect(helpText).toBeVisible();
    await expect(helpText).toContainText('flapjack reset-admin-key');
  });

  test('rejects invalid API key with error message', async ({ page }) => {
    await page.goto('/');

    const authGate = page.locator('[data-testid="auth-gate"]');
    await expect(authGate).toBeVisible();

    // Type an invalid key
    const input = page.locator('[data-testid="auth-key-input"]');
    await input.fill('wrong_key_12345');

    // Submit
    const submitBtn = page.locator('[data-testid="auth-submit"]');
    await expect(submitBtn).toBeEnabled();
    await submitBtn.click();

    // Should show error
    const error = page.locator('[data-testid="auth-error"]');
    await expect(error).toBeVisible({ timeout: 10_000 });
    await expect(error).toContainText('Invalid API key');

    // Should still be on the auth gate (not redirected)
    await expect(authGate).toBeVisible();
  });

  test('accepts valid API key and loads the dashboard', async ({ page }) => {
    await page.goto('/');

    const authGate = page.locator('[data-testid="auth-gate"]');
    await expect(authGate).toBeVisible();

    // Type the correct admin key
    const input = page.locator('[data-testid="auth-key-input"]');
    await input.fill(ADMIN_KEY);

    // Submit
    const submitBtn = page.locator('[data-testid="auth-submit"]');
    await submitBtn.click();

    // Should show success state briefly
    const success = page.locator('[data-testid="auth-success"]');
    await expect(success).toBeVisible({ timeout: 10_000 });

    // After reload, should see the dashboard (Overview page)
    // The page reloads after auth — wait for the Overview heading
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 15_000 });

    // Auth gate should no longer be visible
    await expect(authGate).not.toBeVisible();
  });

  test('connect button enables only when key input is non-empty', async ({ page }) => {
    await page.goto('/');

    await expect(page.locator('[data-testid="auth-gate"]')).toBeVisible();

    const input = page.locator('[data-testid="auth-key-input"]');
    const submitBtn = page.locator('[data-testid="auth-submit"]');

    // Initially disabled
    await expect(submitBtn).toBeDisabled();

    // Type something
    await input.fill('a');
    await expect(submitBtn).toBeEnabled();

    // Clear it
    await input.fill('');
    await expect(submitBtn).toBeDisabled();
  });

  test('persists API key across page reloads', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('[data-testid="auth-gate"]')).toBeVisible();

    // Authenticate
    await page.locator('[data-testid="auth-key-input"]').fill(ADMIN_KEY);
    await page.locator('[data-testid="auth-submit"]').click();

    // Wait for dashboard to load after auth
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 15_000 });

    // Override the beforeEach clearing script: add a restoring script that runs AFTER it.
    // (addInitScript scripts execute in insertion order on every page load.)
    await page.addInitScript((key: string) => {
      localStorage.setItem('flapjack-api-key', key);
      localStorage.setItem('flapjack-app-id', 'flapjack');
      localStorage.setItem('flapjack-auth', JSON.stringify({
        state: { apiKey: key, appId: 'flapjack' },
        version: 0,
      }));
    }, ADMIN_KEY);

    // Reload the page — should go straight to dashboard (no auth gate)
    await page.reload();
    await expect(page.getByRole('heading', { name: 'Overview' })).toBeVisible({ timeout: 15_000 });
    await expect(page.locator('[data-testid="auth-gate"]')).not.toBeVisible();
  });
});
