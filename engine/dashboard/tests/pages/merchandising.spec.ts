import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

const MOCK_SEARCH_RESULTS = {
  hits: [
    { objectID: 'prod-1', name: 'iPhone 15 Pro', brand: 'Apple', _highlightResult: {} },
    { objectID: 'prod-2', name: 'iPhone 15', brand: 'Apple', _highlightResult: {} },
    { objectID: 'prod-3', name: 'Galaxy S24', brand: 'Samsung', _highlightResult: {} },
    { objectID: 'prod-4', name: 'Pixel 8', brand: 'Google', _highlightResult: {} },
    { objectID: 'prod-5', name: 'OnePlus 12', brand: 'OnePlus', _highlightResult: {} },
  ],
  nbHits: 5,
  page: 0,
  nbPages: 1,
  hitsPerPage: 50,
  processingTimeMS: 3,
  query: 'phone',
};

const EMPTY_RULES = { hits: [], nbHits: 0, page: 0, nbPages: 0 };

function mockSearchApi(page: Page) {
  return page.route('**/1/indexes/*/query', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(MOCK_SEARCH_RESULTS),
    });
  });
}

function mockRulesApi(page: Page) {
  return page.route('**/1/indexes/*/rules/search', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(EMPTY_RULES),
    });
  });
}

function mockRuleSave(page: Page) {
  return page.route('**/1/indexes/*/rules/*', (route) => {
    if (route.request().method() === 'PUT') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ objectID: 'rule-new', updatedAt: new Date().toISOString() }),
      });
    } else {
      route.fallback();
    }
  });
}

test.describe('Merchandising Studio — Initial State', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');
  });

  test('shows Merchandising Studio heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: /merchandising studio/i })).toBeVisible();
  });

  test('shows search input placeholder', async ({ page }) => {
    await expect(page.getByPlaceholder(/enter a search query to merchandise/i)).toBeVisible();
  });

  test('shows Search button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /^search$/i })).toBeVisible();
  });

  test('shows instructions sidebar', async ({ page }) => {
    await expect(page.getByText(/how it works/i)).toBeVisible();
    await expect(page.getByText(/pin.*results/i).first()).toBeVisible();
    await expect(page.getByText(/hide.*irrelevant/i).first()).toBeVisible();
    await expect(page.getByText(/save.*as a rule/i).first()).toBeVisible();
  });

  test('shows prompt to enter a search query', async ({ page }) => {
    // The empty-state card has an h3 heading — verify the heading, not the input placeholder
    await expect(page.getByRole('heading', { name: /enter a search query/i })).toBeVisible();
    await expect(page.getByText(/type a query above/i)).toBeVisible();
  });

  test('shows breadcrumb back to Rules', async ({ page }) => {
    await expect(page.getByRole('button', { name: /rules/i })).toBeVisible();
  });

  test('navigates back to rules page via breadcrumb', async ({ page }) => {
    await page.getByRole('button', { name: /rules/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index\/rules/);
  });

  test('does not show Reset or Save buttons initially', async ({ page }) => {
    // Confirm page is loaded before checking negative assertions
    await expect(page.getByRole('heading', { name: /merchandising studio/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /reset/i })).not.toBeVisible();
    await expect(page.getByRole('button', { name: /save as rule/i })).not.toBeVisible();
  });
});

test.describe('Merchandising Studio — Search Results', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');
  });

  test('shows search results after entering a query', async ({ page }) => {
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    // Wait for results
    await expect(page.getByText(/5 results for "phone"/i)).toBeVisible();

    // Verify product cards rendered
    const cards = page.locator('[data-testid="merch-card"]');
    await expect(cards).toHaveCount(5);
  });

  test('shows search results when pressing Enter', async ({ page }) => {
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByPlaceholder(/enter a search query to merchandise/i).press('Enter');

    await expect(page.getByText(/5 results for "phone"/i)).toBeVisible();
  });

  test('renders product objectIDs on result cards', async ({ page }) => {
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    await expect(page.getByText('prod-1')).toBeVisible();
    await expect(page.getByText('prod-2')).toBeVisible();
    await expect(page.getByText('prod-3')).toBeVisible();
  });

  test('renders product field values', async ({ page }) => {
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    await expect(page.getByText('iPhone 15 Pro')).toBeVisible();
    await expect(page.getByText('Galaxy S24')).toBeVisible();
  });

  test('shows processing time', async ({ page }) => {
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    await expect(page.getByText(/3ms/)).toBeVisible();
  });
});

test.describe('Merchandising Studio — Pin & Hide Actions', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockRulesApi(page);
    await mockRuleSave(page);
    await page.goto('/index/test-index/merchandising');

    // Search to get results
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();
    await expect(page.locator('[data-testid="merch-card"]').first()).toBeVisible();
  });

  test('pin button pins an item and shows pin badge', async ({ page }) => {
    const cards = page.locator('[data-testid="merch-card"]');
    const firstCard = cards.first();

    // Click the pin button on the first card
    await firstCard.getByTitle(/pin to this position/i).click();

    // Should show "Pinned" badge
    await expect(firstCard.getByText(/pinned/i)).toBeVisible();
  });

  test('pinning shows status badge in header', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/pin to this position/i).click();

    // Header should now show "1 pinned, 0 hidden"
    await expect(page.getByText(/1 pinned, 0 hidden/i)).toBeVisible();
  });

  test('pinning shows Reset and Save as Rule buttons', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/pin to this position/i).click();

    await expect(page.getByRole('button', { name: /reset/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /save as rule/i })).toBeVisible();
  });

  test('hide button removes item and shows in Hidden section', async ({ page }) => {
    // Initially 5 result cards
    await expect(page.locator('[data-testid="merch-card"]')).toHaveCount(5);

    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/hide from results/i).click();

    // Main results should decrease to 4
    await expect(page.locator('[data-testid="merch-card"]')).toHaveCount(4);

    // Hidden results section should appear
    await expect(page.getByText(/hidden results.*1/i)).toBeVisible();

    // The hidden item's objectID should be shown in the hidden section
    await expect(page.getByText('prod-1')).toBeVisible();
  });

  test('hiding shows status badge in header', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/hide from results/i).click();

    await expect(page.getByText(/0 pinned, 1 hidden/i)).toBeVisible();
  });

  test('unhide button restores hidden item', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/hide from results/i).click();

    // Verify hidden section exists
    await expect(page.getByText(/hidden results/i)).toBeVisible();

    // Click unhide
    await page.getByTitle(/unhide/i).click();

    // Hidden section should disappear
    await expect(page.getByText(/hidden results/i)).not.toBeVisible();
  });

  test('pin shows position badge and reorder buttons', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/pin to this position/i).click();

    // Pinned card should show position badge (#1) and reorder buttons (Move up + Move down)
    const pinnedCard = page.locator('[data-testid="merch-card"]').first();
    await expect(pinnedCard.getByText('#1')).toBeVisible();
    await expect(pinnedCard.getByTitle('Move up')).toBeVisible();
    await expect(pinnedCard.getByTitle('Move down')).toBeVisible();
  });

  test('reset clears all pins and hides', async ({ page }) => {
    // Pin first item
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/pin to this position/i).click();
    await expect(page.getByText(/1 pinned/i)).toBeVisible();

    // Click Reset
    await page.getByRole('button', { name: /reset/i }).click();

    // Status badge should disappear
    await expect(page.getByRole('button', { name: /reset/i })).not.toBeVisible();
    await expect(page.getByRole('button', { name: /save as rule/i })).not.toBeVisible();
  });

  test('shows rule description input when changes exist', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();
    await firstCard.getByTitle(/pin to this position/i).click();

    // Rule description field should appear
    await expect(page.getByText(/rule description/i)).toBeVisible();
    await expect(page.getByPlaceholder(/merchandising.*phone/i)).toBeVisible();
  });
});

test.describe('Merchandising Studio — Save as Rule', () => {
  test('save sends correct rule data to API', async ({ page }) => {
    let savedRule: any = null;
    await mockSearchApi(page);
    await mockRulesApi(page);
    await page.route('**/1/indexes/*/rules/*', (route) => {
      if (route.request().url().includes('/rules/search')) {
        return route.fallback();
      }
      if (route.request().method() === 'PUT') {
        savedRule = route.request().postDataJSON();
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ objectID: savedRule?.objectID, updatedAt: new Date().toISOString() }),
        });
      } else {
        route.fallback();
      }
    });

    await page.goto('/index/test-index/merchandising');

    // Search
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    // Wait for result cards to appear
    const cards = page.locator('[data-testid="merch-card"]');
    await expect(cards.first()).toBeVisible();

    // Pin first result — wait for the pin badge to confirm state change
    await cards.first().getByTitle(/pin to this position/i).click();
    await expect(page.getByText(/1 pinned/i)).toBeVisible();

    // Click Save as Rule and capture the API call
    const responsePromise = page.waitForResponse(
      (response) => response.url().includes('/rules/') && response.request().method() === 'PUT'
    );
    await page.getByRole('button', { name: /save as rule/i }).click();
    await responsePromise;

    // Verify the rule structure
    expect(savedRule).toBeTruthy();
    expect(savedRule.conditions).toHaveLength(1);
    expect(savedRule.conditions[0].pattern).toBe('phone');
    expect(savedRule.conditions[0].anchoring).toBe('is');
    expect(savedRule.consequence.promote).toHaveLength(1);
    expect(savedRule.consequence.promote[0].objectID).toBe('prod-1');
    expect(savedRule.enabled).toBe(true);
  });
});

// ─── Unpin Action ───────────────────────────────────────────────────────────

test.describe('Merchandising Studio — Unpin Action', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();
    await expect(page.locator('[data-testid="merch-card"]').first()).toBeVisible();
  });

  test('clicking pin button on a pinned item unpins it', async ({ page }) => {
    const firstCard = page.locator('[data-testid="merch-card"]').first();

    // Pin the item
    await firstCard.getByTitle(/pin to this position/i).click();
    await expect(firstCard.getByText(/pinned/i)).toBeVisible();
    await expect(page.getByText(/1 pinned/i)).toBeVisible();

    // Unpin by clicking pin button again (now shows "Unpin" title)
    await firstCard.getByTitle(/unpin/i).click();

    // Pin badge should disappear and status should clear
    await expect(page.getByRole('button', { name: /reset/i })).not.toBeVisible();
  });
});

// ─── Move Up / Move Down ────────────────────────────────────────────────────

test.describe('Merchandising Studio — Move Position', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();
    await expect(page.locator('[data-testid="merch-card"]').first()).toBeVisible();
  });

  test('clicking Move down on pinned item changes its position', async ({ page }) => {
    const cards = page.locator('[data-testid="merch-card"]');

    // Pin the first item (position 0)
    await cards.first().getByTitle(/pin to this position/i).click();
    await expect(cards.first().getByText(/pinned #0/i)).toBeVisible();

    // Click Move down
    await cards.first().getByTitle('Move down').click();

    // Position should update to #1
    await expect(page.getByText(/pinned #1/i)).toBeVisible();
  });

  test('clicking Move up on pinned item decreases its position', async ({ page }) => {
    const cards = page.locator('[data-testid="merch-card"]');

    // Pin the second item (position 1)
    await cards.nth(1).getByTitle(/pin to this position/i).click();
    await expect(page.getByText(/pinned #1/i)).toBeVisible();

    // Click Move up
    const pinnedCard = page.locator('[data-testid="merch-card"]').filter({ hasText: /pinned/i });
    await pinnedCard.getByTitle('Move up').click();

    // Position should decrease to #0
    await expect(page.getByText(/pinned #0/i)).toBeVisible();
  });
});

// ─── Multiple Pins + Hides ──────────────────────────────────────────────────

test.describe('Merchandising Studio — Multiple Actions', () => {
  test.beforeEach(async ({ page }) => {
    await mockSearchApi(page);
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();
    await expect(page.locator('[data-testid="merch-card"]').first()).toBeVisible();
  });

  test('multiple pins and hides update status badge correctly', async ({ page }) => {
    const cards = page.locator('[data-testid="merch-card"]');

    // Pin first two items
    await cards.first().getByTitle(/pin to this position/i).click();
    await expect(page.getByText(/1 pinned, 0 hidden/i)).toBeVisible();

    await cards.nth(1).getByTitle(/pin to this position/i).click();
    await expect(page.getByText(/2 pinned, 0 hidden/i)).toBeVisible();

    // Hide third item
    await cards.nth(2).getByTitle(/hide from results/i).click();
    await expect(page.getByText(/2 pinned, 1 hidden/i)).toBeVisible();
  });
});

// ─── Save as Rule with Hidden Items ─────────────────────────────────────────

test.describe('Merchandising Studio — Save with Hides', () => {
  test('save sends rule with consequence.hide for hidden items', async ({ page }) => {
    let savedRule: any = null;
    await mockSearchApi(page);
    await mockRulesApi(page);
    await page.route('**/1/indexes/*/rules/*', (route) => {
      if (route.request().url().includes('/rules/search')) {
        return route.fallback();
      }
      if (route.request().method() === 'PUT') {
        savedRule = route.request().postDataJSON();
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ objectID: savedRule?.objectID, updatedAt: new Date().toISOString() }),
        });
      } else {
        route.fallback();
      }
    });

    await page.goto('/index/test-index/merchandising');
    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();
    await expect(page.locator('[data-testid="merch-card"]').first()).toBeVisible();

    // Hide the first item
    await page.locator('[data-testid="merch-card"]').first().getByTitle(/hide from results/i).click();
    await expect(page.getByText(/0 pinned, 1 hidden/i)).toBeVisible();

    // Save
    const responsePromise = page.waitForResponse(
      (response) => response.url().includes('/rules/') && response.request().method() === 'PUT'
    );
    await page.getByRole('button', { name: /save as rule/i }).click();
    await responsePromise;

    expect(savedRule).toBeTruthy();
    expect(savedRule.consequence.hide).toHaveLength(1);
    expect(savedRule.consequence.hide[0].objectID).toBe('prod-1');
  });
});

// ─── Existing Rules Sidebar ─────────────────────────────────────────────────

test.describe('Merchandising Studio — Existing Rules', () => {
  test('shows existing rules sidebar when rules exist for query', async ({ page }) => {
    await mockSearchApi(page);
    // Return rules instead of empty
    await page.route('**/1/indexes/*/rules/search', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          hits: [
            {
              objectID: 'merch-phone-123',
              description: 'Phone merchandising rule',
              conditions: [{ pattern: 'phone', anchoring: 'is' }],
              consequence: { promote: [{ objectID: 'prod-1', position: 0 }] },
              enabled: true,
            },
          ],
          nbHits: 1,
          page: 0,
          nbPages: 1,
        }),
      });
    });
    await page.goto('/index/test-index/merchandising');

    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    // Existing Rules card should appear in sidebar
    await expect(page.getByText('Existing Rules')).toBeVisible();
    await expect(page.getByText('merch-phone-123')).toBeVisible();
    await expect(page.getByText('1 pinned')).toBeVisible();
  });
});

// ─── Empty Results ──────────────────────────────────────────────────────────

test.describe('Merchandising Studio — Empty Results', () => {
  test('shows "No results" when search returns empty hits', async ({ page }) => {
    await page.route('**/1/indexes/*/query', (route) => {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          hits: [],
          nbHits: 0,
          page: 0,
          nbPages: 0,
          hitsPerPage: 50,
          processingTimeMS: 1,
          query: 'zzzzz',
        }),
      });
    });
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');

    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('zzzzz');
    await page.getByRole('button', { name: /^search$/i }).click();

    await expect(page.getByText('No results')).toBeVisible();
  });
});

// ─── Loading State ──────────────────────────────────────────────────────────

test.describe('Merchandising Studio — Loading State', () => {
  test('shows "Searching..." while waiting for results', async ({ page }) => {
    await page.route('**/1/indexes/*/query', async (route) => {
      await new Promise((r) => setTimeout(r, 2000));
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_SEARCH_RESULTS),
      });
    });
    await mockRulesApi(page);
    await page.goto('/index/test-index/merchandising');

    await page.getByPlaceholder(/enter a search query to merchandise/i).fill('phone');
    await page.getByRole('button', { name: /^search$/i }).click();

    await expect(page.getByText('Searching...')).toBeVisible();
  });
});