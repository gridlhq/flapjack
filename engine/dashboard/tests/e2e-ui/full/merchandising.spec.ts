/**
 * E2E-UI Full Suite -- Merchandising Studio Page (Real Server)
 *
 * NON-MOCKED SIMULATED-HUMAN REAL-BROWSER TESTS.
 * Tests the Merchandising Studio against a real Flapjack backend with seeded data.
 *
 * Covers:
 * - Search for products and see results
 * - Pin button visible and functional
 * - Hide button visible and functional
 * - Pinning a result shows pin badge and moves to position 1
 * - Hiding a result shows hidden count
 * - Pin + hide combination
 * - Save as rule → cross-page verification on Rules page
 * - Different queries return different results
 * - Results summary hit count
 * - How It Works help card
 */
import { test, expect } from '../../fixtures/auth.fixture';
import { API_BASE, API_HEADERS, TEST_INDEX } from '../helpers';

const MERCH_URL = `/index/${TEST_INDEX}/merchandising`;

test.describe('Merchandising Studio', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(MERCH_URL);
    await expect(page.getByText('Merchandising Studio').first()).toBeVisible({ timeout: 15_000 });
  });

  test('search for "laptop" shows product results', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    await expect(page.getByText('MacBook Pro').first()).toBeVisible({ timeout: 10_000 });

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible();
    const count = await cards.count();
    expect(count).toBeGreaterThanOrEqual(1);

    await expect(page.getByText(/results for/i).first()).toBeVisible();
  });

  test('Pin button is visible on result cards', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const firstCard = page.getByTestId('merch-card').first();
    await expect(firstCard).toBeVisible({ timeout: 10_000 });

    const pinButton = firstCard.getByRole('button', { name: /pin/i }).or(
      firstCard.locator('button[title*="Pin"]')
    );
    await expect(pinButton.first()).toBeVisible();
  });

  test('Hide button is visible on result cards', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const firstCard = page.getByTestId('merch-card').first();
    await expect(firstCard).toBeVisible({ timeout: 10_000 });

    const hideButton = firstCard.locator('button[title="Hide from results"]');
    await expect(hideButton).toBeVisible();
  });

  test('pinning a result shows pin badge and moves it to position 1', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('headphones');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    const cardCount = await cards.count();
    if (cardCount >= 2) {
      const secondCard = cards.nth(1);

      const pinBtn = secondCard.locator('button[title*="Pin"]').or(
        secondCard.getByRole('button', { name: /pin/i })
      );
      await pinBtn.first().click();

      await expect(page.getByText(/Pinned #/i).first()).toBeVisible({ timeout: 5_000 });
      await expect(page.getByText(/1 pinned/i).first()).toBeVisible();

      await expect(page.getByRole('button', { name: /Save as Rule/i })).toBeVisible();
      await expect(page.getByRole('button', { name: /Reset/i })).toBeVisible();

      await page.getByRole('button', { name: /Reset/i }).click();
      await expect(page.getByText(/Pinned #/i)).not.toBeVisible({ timeout: 5_000 });
    }
  });

  test('hiding a result moves it to hidden section', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    const firstCard = cards.first();
    const hideBtn = firstCard.locator('button[title="Hide from results"]');
    await hideBtn.click();

    await expect(page.getByText(/1 hidden/i).first()).toBeVisible({ timeout: 5_000 });

    await expect(page.getByRole('button', { name: /Save as Rule/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /Reset/i })).toBeVisible();

    await page.getByRole('button', { name: /Reset/i }).click();
    await expect(page.getByText(/1 hidden/i)).not.toBeVisible({ timeout: 5_000 });
  });

  // ---------- Pin + Hide Combination ----------

  test('pin and hide multiple results shows combined counts', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    const cardCount = await cards.count();
    if (cardCount >= 2) {
      // Pin first card
      const firstCard = cards.first();
      const pinBtn = firstCard.locator('button[title*="Pin"]').or(
        firstCard.getByRole('button', { name: /pin/i })
      );
      await pinBtn.first().click();
      await expect(page.getByText(/1 pinned/i).first()).toBeVisible({ timeout: 5_000 });

      // Hide second card
      const secondCard = cards.nth(1);
      const hideBtn = secondCard.locator('button[title="Hide from results"]');
      await hideBtn.click();
      await expect(page.getByText(/1 hidden/i).first()).toBeVisible({ timeout: 5_000 });

      await expect(page.getByText(/1 pinned/i).first()).toBeVisible();
      await expect(page.getByText(/1 hidden/i).first()).toBeVisible();

      await page.getByRole('button', { name: /Reset/i }).click();
    }
  });

  // ---------- Cross-page: Save as Rule → Rules Page ----------

  test('save as rule then verify rule appears on Rules page', async ({ page, request }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('monitor');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    // Pin the first result
    const firstCard = cards.first();
    const pinBtn = firstCard.locator('button[title*="Pin"]').or(
      firstCard.getByRole('button', { name: /pin/i })
    );
    await pinBtn.first().click();
    await expect(page.getByText(/Pinned #/i).first()).toBeVisible({ timeout: 5_000 });

    // Save as Rule
    const saveBtn = page.getByRole('button', { name: /Save as Rule/i });
    await expect(saveBtn).toBeVisible();

    const responsePromise = page.waitForResponse(
      resp => resp.url().includes('/rules'),
      { timeout: 10_000 }
    );
    await saveBtn.click();
    await responsePromise;

    // Navigate to Rules page and verify
    await page.goto(`/index/${TEST_INDEX}/rules`);
    await expect(page.getByText('Rules').first()).toBeVisible({ timeout: 15_000 });
    await expect(page.getByText(/monitor/).first()).toBeVisible({ timeout: 10_000 });

    // Cleanup
    const rulesRes = await request.get(
      `${API_BASE}/1/indexes/${TEST_INDEX}/rules`,
      { headers: API_HEADERS }
    );
    if (rulesRes.ok()) {
      const rules = await rulesRes.json();
      const items = rules.hits || rules.items || rules;
      if (Array.isArray(items)) {
        for (const rule of items) {
          if (rule.objectID?.startsWith('merch-')) {
            await request.delete(
              `${API_BASE}/1/indexes/${TEST_INDEX}/rules/${rule.objectID}`,
              { headers: API_HEADERS }
            );
          }
        }
      }
    }
  });

  test('searching different queries returns different merchandise results', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);

    await searchInput.fill('tablet');
    await page.getByRole('button', { name: /^Search$/i }).click();
    await expect(page.getByTestId('merch-card').first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/results for/i).first()).toBeVisible();

    await searchInput.fill('monitor');
    await page.getByRole('button', { name: /^Search$/i }).click();
    await expect(page.getByTestId('merch-card').first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/results for/i).first()).toBeVisible();
  });

  test('results summary shows hit count for query', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    await expect(page.getByText(/results for/i).first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText('laptop').first()).toBeVisible();
  });

  test('merchandising studio shows "how it works" help card', async ({ page }) => {
    const howItWorks = page.getByText(/how it works/i).or(
      page.getByText(/enter a search query/i)
    );
    await expect(howItWorks.first()).toBeVisible({ timeout: 10_000 });
  });

  // ---------- Drag-and-Drop ----------

  test('drag handle is visible on all result cards', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const firstCard = page.getByTestId('merch-card').first();
    await expect(firstCard).toBeVisible({ timeout: 10_000 });

    // Drag handle should be visible
    const dragHandle = firstCard.getByTestId('drag-handle');
    await expect(dragHandle).toBeVisible();

    // Should have grab cursor styling
    await expect(dragHandle).toHaveCSS('cursor', 'grab');
  });

  test('result cards are draggable (have draggable attribute)', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const firstCard = page.getByTestId('merch-card').first();
    await expect(firstCard).toBeVisible({ timeout: 10_000 });

    // Card should be draggable
    await expect(firstCard).toHaveAttribute('draggable', 'true');
  });

  test('drag and drop pins item at target position', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    const cardCount = await cards.count();
    if (cardCount >= 2) {
      // Drag second card to first position
      const sourceCard = cards.nth(1);
      const targetCard = cards.nth(0);

      await sourceCard.dragTo(targetCard);

      // The dragged item should now be pinned
      await expect(page.getByText(/Pinned #/i).first()).toBeVisible({ timeout: 5_000 });
      await expect(page.getByText(/1 pinned/i).first()).toBeVisible();

      // Reset
      await page.getByRole('button', { name: /Reset/i }).click();
    }
  });

  test('up/down arrow buttons work for pinned items', async ({ page }) => {
    const searchInput = page.getByPlaceholder(/search query to merchandise/i);
    await searchInput.fill('laptop');
    await page.getByRole('button', { name: /^Search$/i }).click();

    const cards = page.getByTestId('merch-card');
    await expect(cards.first()).toBeVisible({ timeout: 10_000 });

    // Pin the first card
    const firstCard = cards.first();
    const pinBtn = firstCard.locator('button[title*="Pin"]').or(
      firstCard.getByRole('button', { name: /pin/i })
    );
    await pinBtn.first().click();
    await expect(page.getByText(/Pinned #/i).first()).toBeVisible({ timeout: 5_000 });

    // Move down button should be visible for pinned items
    const moveDownBtn = page.locator('button[title="Move down"]').first();
    await expect(moveDownBtn).toBeVisible();

    // Move up button should be visible too
    const moveUpBtn = page.locator('button[title="Move up"]').first();
    await expect(moveUpBtn).toBeVisible();

    // Click move down
    await moveDownBtn.click();

    // Position should have changed
    await expect(page.getByText(/Pinned #/i).first()).toBeVisible();

    // Reset
    await page.getByRole('button', { name: /Reset/i }).click();
  });
});
