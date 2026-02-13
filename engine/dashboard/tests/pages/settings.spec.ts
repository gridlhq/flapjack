import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

const MOCK_SETTINGS = {
  searchableAttributes: ['title', 'description', 'brand'],
  attributesForFaceting: ['category', 'brand', 'price'],
  ranking: ['typo', 'geo', 'words', 'filters', 'proximity', 'attribute', 'exact', 'custom'],
  customRanking: ['desc(popularity)'],
  hitsPerPage: 20,
  highlightPreTag: '<em>',
  highlightPostTag: '</em>',
  queryType: 'prefixLast',
  minWordSizefor1Typo: 4,
  minWordSizefor2Typos: 8,
};

/** Mock the GET and PUT /1/indexes/{name}/settings endpoints. */
async function mockSettingsApi(page: Page, response = MOCK_SETTINGS) {
  await page.route('**/1/indexes/*/settings', (route) => {
    if (route.request().method() === 'GET') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(response),
      });
    } else if (route.request().method() === 'PUT') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ ...response, ...route.request().postDataJSON() }),
      });
    } else {
      route.fallback();
    }
  });
}

/** Mock the POST /1/indexes/{name}/compact endpoint. */
async function mockCompactApi(page: Page) {
  await page.route('**/1/indexes/*/compact', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ taskID: 1 }),
    });
  });
}

/**
 * Mock the POST /1/indexes/{name}/query endpoint used by useIndexFields.
 * Returns a sample hit so the SettingsForm can derive available field chips.
 */
async function mockFieldsApi(page: Page) {
  await page.route('**/1/indexes/*/query', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hits: [
          {
            objectID: 'doc-1',
            title: 'Sample Product',
            description: 'A great product',
            brand: 'Acme',
            category: 'Widgets',
            price: 9.99,
          },
        ],
        nbHits: 1,
        page: 0,
        nbPages: 1,
        hitsPerPage: 1,
      }),
    });
  });
}

// ---------------------------------------------------------------------------
// Settings Page -- With Data
// ---------------------------------------------------------------------------
test.describe('Settings Page — With Data', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('displays Settings heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();
  });

  test('shows breadcrumb with index name', async ({ page }) => {
    await expect(page.getByRole('button', { name: /test-index/i })).toBeVisible();
  });

  test('renders Searchable Attributes from API data', async ({ page }) => {
    await expect(page.getByText('Searchable Attributes')).toBeVisible();

    // The textarea should contain the comma-separated searchable attributes
    const textarea = page.locator('textarea').first();
    await expect(textarea).toHaveValue('title, description, brand');
  });

  test('renders Hits Per Page value from API data', async ({ page }) => {
    const hitsInput = page.getByPlaceholder('20');
    await expect(hitsInput).toBeVisible();
    await expect(hitsInput).toHaveValue('20');
  });

  test('renders Faceting section with attributes', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Faceting' })).toBeVisible();
    await expect(page.getByText('Attributes For Faceting')).toBeVisible();

    // Find the Attributes For Faceting textarea (second textarea set in the form)
    // The faceting textarea should show the comma-separated values
    await expect(async () => {
      const textareas = page.locator('textarea');
      const count = await textareas.count();
      let found = false;
      for (let i = 0; i < count; i++) {
        const value = await textareas.nth(i).inputValue();
        if (value === 'category, brand, price') {
          found = true;
          break;
        }
      }
      expect(found).toBe(true);
    }).toPass();
  });

  test('shows Compact Index button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /compact index/i })).toBeVisible();
  });

  test('shows JSON toggle button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /json/i })).toBeVisible();
  });

  test('clicking JSON toggle shows JSON editor with settings data', async ({ page }) => {
    await page.getByRole('button', { name: /json/i }).click();

    // The JSON view should contain the raw settings text
    await expect(page.getByText('"searchableAttributes"')).toBeVisible();
    await expect(page.getByText('"hitsPerPage"')).toBeVisible();
  });

  test('clicking JSON toggle again returns to form view', async ({ page }) => {
    const jsonButton = page.getByRole('button', { name: /json/i });

    // Toggle on
    await jsonButton.click();
    await expect(page.getByText('"searchableAttributes"')).toBeVisible();

    // Toggle off -- form should reappear
    await jsonButton.click();
    await expect(page.getByText('Search Behavior')).toBeVisible();
    await expect(page.getByText('Searchable Attributes')).toBeVisible();
  });

  test('Save and Reset buttons are hidden when form is clean', async ({ page }) => {
    // Wait for the form to be rendered
    await expect(page.getByText('Search Behavior')).toBeVisible();

    // Save and Reset should not be visible on a clean form
    await expect(page.getByRole('button', { name: /save changes/i })).not.toBeVisible();
    await expect(page.getByRole('button', { name: /reset/i })).not.toBeVisible();
  });

  test('modifying Hits Per Page input shows Save and Reset buttons', async ({ page }) => {
    const hitsInput = page.getByPlaceholder('20');
    await expect(hitsInput).toBeVisible();

    await hitsInput.fill('25');

    await expect(page.getByRole('button', { name: /save changes/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /reset/i })).toBeVisible();
  });

  test('clicking Reset after modification reverts the value and hides Save/Reset', async ({ page }) => {
    const hitsInput = page.getByPlaceholder('20');
    await expect(hitsInput).toHaveValue('20');

    // Modify the field
    await hitsInput.fill('25');
    await expect(page.getByRole('button', { name: /reset/i })).toBeVisible();

    // Click Reset
    await page.getByRole('button', { name: /reset/i }).click();

    // Value should revert to the original and Save/Reset should disappear
    await expect(hitsInput).toHaveValue('20');
    await expect(page.getByRole('button', { name: /save changes/i })).not.toBeVisible();
    await expect(page.getByRole('button', { name: /reset/i })).not.toBeVisible();
  });

  test('clicking Save sends PUT request and receives 200', async ({ page }) => {
    const hitsInput = page.getByPlaceholder('20');
    await expect(hitsInput).toBeVisible();

    await hitsInput.fill('50');
    await expect(page.getByRole('button', { name: /save changes/i })).toBeVisible();

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/1/indexes/test-index/settings') &&
        response.request().method() === 'PUT' &&
        response.status() === 200,
    );

    await page.getByRole('button', { name: /save changes/i }).click();
    await responsePromise;
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Save Sends Correct Data
// ---------------------------------------------------------------------------
test.describe('Settings Page — Save Sends Correct Data', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('PUT request body contains the modified field', async ({ page }) => {
    let lastPutBody: any = null;

    // Intercept PUT to capture the body
    await page.route('**/1/indexes/*/settings', (route) => {
      if (route.request().method() === 'PUT') {
        lastPutBody = route.request().postDataJSON();
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ ...MOCK_SETTINGS, ...lastPutBody }),
        });
      } else {
        route.fallback();
      }
    });

    const hitsInput = page.getByPlaceholder('20');
    await expect(hitsInput).toHaveValue('20');

    await hitsInput.fill('42');
    await expect(page.getByRole('button', { name: /save changes/i })).toBeVisible();

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/settings') &&
        response.request().method() === 'PUT' &&
        response.status() === 200,
    );

    await page.getByRole('button', { name: /save changes/i }).click();
    await responsePromise;

    expect(lastPutBody).not.toBeNull();
    expect(lastPutBody.hitsPerPage).toBe(42);
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Compact Index
// ---------------------------------------------------------------------------
test.describe('Settings Page — Compact Index', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('clicking Compact Index sends POST to compact endpoint', async ({ page }) => {
    await expect(page.getByRole('button', { name: /compact index/i })).toBeVisible();

    const responsePromise = page.waitForResponse(
      (response) =>
        response.url().includes('/1/indexes/test-index/compact') &&
        response.request().method() === 'POST' &&
        response.status() === 200,
    );

    await page.getByRole('button', { name: /compact index/i }).click();
    await responsePromise;
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Navigation
// ---------------------------------------------------------------------------
test.describe('Settings Page — Navigation', () => {
  test('clicking breadcrumb navigates back to /index/test-index', async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');

    const breadcrumb = page.getByRole('button', { name: /test-index/i });
    await expect(breadcrumb).toBeVisible();
    await breadcrumb.click();

    await expect(page).toHaveURL(/\/index\/test-index$/);
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Ranking & Sorting Section
// ---------------------------------------------------------------------------
test.describe('Settings Page — Ranking & Sorting', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('renders Ranking & Sorting section heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Ranking & Sorting' })).toBeVisible();
  });

  test('shows ranking criteria textarea with values from API', async ({ page }) => {
    await expect(page.getByText('Ranking Criteria', { exact: true })).toBeVisible();
    await expect(async () => {
      const textareas = page.locator('textarea');
      const count = await textareas.count();
      let found = false;
      for (let i = 0; i < count; i++) {
        const value = await textareas.nth(i).inputValue();
        if (value.includes('typo') && value.includes('proximity')) {
          found = true;
          break;
        }
      }
      expect(found).toBe(true);
    }).toPass();
  });

  test('shows custom ranking textarea with values from API', async ({ page }) => {
    await expect(page.getByText('Custom Ranking', { exact: true })).toBeVisible();
    await expect(async () => {
      const textareas = page.locator('textarea');
      const count = await textareas.count();
      let found = false;
      for (let i = 0; i < count; i++) {
        const value = await textareas.nth(i).inputValue();
        if (value.includes('desc(popularity)')) {
          found = true;
          break;
        }
      }
      expect(found).toBe(true);
    }).toPass();
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Display & Highlighting Section
// ---------------------------------------------------------------------------
test.describe('Settings Page — Display & Highlighting', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('renders Display & Highlighting section heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Display & Highlighting' })).toBeVisible();
  });

  test('shows highlight pre tag input with value', async ({ page }) => {
    await expect(page.getByText('Highlight Pre Tag')).toBeVisible();
    const input = page.locator('input[placeholder="<em>"]');
    await expect(input).toHaveValue('<em>');
  });

  test('shows highlight post tag input with value', async ({ page }) => {
    await expect(page.getByText('Highlight Post Tag')).toBeVisible();
    const input = page.locator('input[placeholder="</em>"]');
    await expect(input).toHaveValue('</em>');
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Advanced Section
// ---------------------------------------------------------------------------
test.describe('Settings Page — Advanced', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('renders Advanced section heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Advanced' })).toBeVisible();
  });

  test('shows Remove Stop Words toggle', async ({ page }) => {
    await expect(page.getByText('Remove Stop Words')).toBeVisible();
  });

  test('shows Ignore Plurals toggle', async ({ page }) => {
    await expect(page.getByText('Ignore Plurals')).toBeVisible();
  });

  test('shows Min Word Size for 1 Typo input with value 4', async ({ page }) => {
    await expect(page.getByText('Min Word Size for 1 Typo')).toBeVisible();
    const input = page.locator('input[placeholder="4"]');
    await expect(input).toHaveValue('4');
  });

  test('shows Min Word Size for 2 Typos input with value 8', async ({ page }) => {
    await expect(page.getByText('Min Word Size for 2 Typos')).toBeVisible();
    const input = page.locator('input[placeholder="8"]');
    await expect(input).toHaveValue('8');
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Field Chips Interaction
// ---------------------------------------------------------------------------
test.describe('Settings Page — Field Chips', () => {
  test.beforeEach(async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');
  });

  test('shows field chips for available fields', async ({ page }) => {
    await expect(page.locator('button.rounded-full', { hasText: 'title' }).first()).toBeVisible();
  });

  test('clicking a chip toggles selection and shows Save/Reset', async ({ page }) => {
    const chip = page.locator('button.rounded-full', { hasText: 'category' }).first();
    await chip.click();
    await expect(page.getByRole('button', { name: /save changes/i })).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Loading State
// ---------------------------------------------------------------------------
test.describe('Settings Page — Loading State', () => {
  test('shows skeleton cards while loading settings', async ({ page }) => {
    await page.route('**/1/indexes/*/settings', async (route) => {
      await new Promise((r) => setTimeout(r, 2000));
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_SETTINGS),
      });
    });
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');

    const skeletons = page.locator('.animate-pulse');
    await expect(skeletons.first()).toBeVisible();
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Re-index Warning
// ---------------------------------------------------------------------------
test.describe('Settings Page — Re-index Warning', () => {
  test('changing facet attributes shows "Reindex needed" badge and Re-index now button', async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');

    // Initially faceting should show "Up to date"
    await expect(page.getByText('Up to date')).toBeVisible();
    await expect(page.getByText('Reindex needed')).not.toBeVisible();

    // Modify the faceting textarea (remove "price" from the list)
    const facetingSection = page.getByRole('heading', { name: 'Faceting' }).locator('..');
    const allTextareas = page.locator('textarea');
    // Find the faceting textarea (contains "category, brand, price")
    let facetTextarea = allTextareas.first();
    const count = await allTextareas.count();
    for (let i = 0; i < count; i++) {
      const value = await allTextareas.nth(i).inputValue();
      if (value === 'category, brand, price') {
        facetTextarea = allTextareas.nth(i);
        break;
      }
    }
    await facetTextarea.fill('category, brand');

    // "Reindex needed" badge and "Re-index now" button should appear
    await expect(page.getByText('Reindex needed')).toBeVisible();
    await expect(page.getByRole('button', { name: /re-index now/i })).toBeVisible();
  });

  test('clicking "Re-index now" opens confirm dialog', async ({ page }) => {
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');

    // Wait for page to fully load
    await expect(page.getByText('Up to date')).toBeVisible();

    // Change facets to trigger the warning
    const allTextareas = page.locator('textarea');
    let facetTextarea = allTextareas.first();
    const count = await allTextareas.count();
    for (let i = 0; i < count; i++) {
      const value = await allTextareas.nth(i).inputValue();
      if (value === 'category, brand, price') {
        facetTextarea = allTextareas.nth(i);
        break;
      }
    }
    await facetTextarea.fill('category');

    // Click the Re-index now button
    await expect(page.getByRole('button', { name: /re-index now/i })).toBeVisible();
    await page.getByRole('button', { name: /re-index now/i }).click();

    // Confirm dialog should appear
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(page.getByText('Re-index All Documents')).toBeVisible();
    await expect(dialog.getByRole('button', { name: /re-index$/i })).toBeVisible();
  });

  test('confirming re-index sends POST to browse endpoint', async ({ page }) => {
    let browseRequested = false;
    await page.route('**/1/indexes/*/browse', (route) => {
      browseRequested = true;
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ hits: [], cursor: null, nbHits: 0 }),
      });
    });
    await mockSettingsApi(page);
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');

    // Wait for page to fully load
    await expect(page.getByText('Up to date')).toBeVisible();

    // Change facets
    const allTextareas = page.locator('textarea');
    let facetTextarea = allTextareas.first();
    const count = await allTextareas.count();
    for (let i = 0; i < count; i++) {
      const value = await allTextareas.nth(i).inputValue();
      if (value === 'category, brand, price') {
        facetTextarea = allTextareas.nth(i);
        break;
      }
    }
    await facetTextarea.fill('category');

    await expect(page.getByRole('button', { name: /re-index now/i })).toBeVisible();
    await page.getByRole('button', { name: /re-index now/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Confirm
    await dialog.getByRole('button', { name: /re-index$/i }).click();

    // Should trigger a browse request (first step of reindex)
    await expect.poll(() => browseRequested).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Settings Page -- Save Error Handling
// ---------------------------------------------------------------------------
test.describe('Settings Page — Save Error Handling', () => {
  test('handles 500 error on save gracefully', async ({ page }) => {
    // GET returns normally, but PUT returns 500
    await page.route('**/1/indexes/*/settings', (route) => {
      if (route.request().method() === 'GET') {
        route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(MOCK_SETTINGS),
        });
      } else if (route.request().method() === 'PUT') {
        route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'Internal Server Error' }),
        });
      } else {
        route.fallback();
      }
    });
    await mockCompactApi(page);
    await mockFieldsApi(page);
    await page.goto('/index/test-index/settings');

    const hitsInput = page.getByPlaceholder('20');
    await expect(hitsInput).toHaveValue('20');

    await hitsInput.fill('50');
    await expect(page.getByRole('button', { name: /save changes/i })).toBeVisible();

    // Click Save — it should fail but not crash
    await page.getByRole('button', { name: /save changes/i }).click();

    // Save button should remain visible since the save failed (form still dirty)
    await expect(page.getByRole('button', { name: /save changes/i })).toBeVisible();
  });
});
