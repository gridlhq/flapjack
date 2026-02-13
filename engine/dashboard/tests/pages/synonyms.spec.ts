import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

// Mock data matching the Algolia synonyms API shape
const MOCK_SYNONYMS = {
  hits: [
    { type: 'synonym', objectID: 'syn-1', synonyms: ['hoodie', 'sweatshirt', 'pullover'] },
    { type: 'onewaysynonym', objectID: 'syn-2', input: 'phone', synonyms: ['iphone', 'smartphone'] },
    { type: 'altcorrection1', objectID: 'syn-3', word: 'tshirt', corrections: ['t-shirt', 'tee shirt'] },
    { type: 'placeholder', objectID: 'syn-4', placeholder: 'brand', replacements: ['nike', 'adidas'] },
  ],
  nbHits: 4,
};

const EMPTY_SYNONYMS = { hits: [], nbHits: 0 };

function mockSynonymsApi(page: Page, response = MOCK_SYNONYMS) {
  return page.route('**/1/indexes/*/synonyms/search', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(response),
    });
  });
}

function mockSynonymSave(page: Page) {
  return page.route('**/1/indexes/*/synonyms/*', (route) => {
    // Don't intercept the /synonyms/search endpoint — let the search mock handle it
    if (route.request().url().includes('/synonyms/search')) {
      return route.fallback();
    }
    if (route.request().method() === 'PUT') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ objectID: 'syn-new', updatedAt: new Date().toISOString() }),
      });
    } else if (route.request().method() === 'DELETE') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ deletedAt: new Date().toISOString() }),
      });
    } else {
      route.fallback();
    }
  });
}

test.describe('Synonyms Page — Empty State', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page, EMPTY_SYNONYMS);
    await page.goto('/index/test-index/synonyms');
  });

  test('shows empty state message when no synonyms exist', async ({ page }) => {
    await expect(page.getByText(/no synonyms/i)).toBeVisible();
    await expect(page.getByText(/synonyms help users find results/i)).toBeVisible();
  });

  test('shows quick-create buttons for Multi-way and One-way in empty state', async ({ page }) => {
    await expect(page.getByText(/no synonyms/i)).toBeVisible();
    await expect(page.getByRole('button', { name: /multi-way/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /one-way/i })).toBeVisible();
  });

  test('does not show Clear All button when no synonyms exist', async ({ page }) => {
    await expect(page.getByText(/no synonyms/i)).toBeVisible();
    await expect(page.getByRole('button', { name: /clear all/i })).not.toBeVisible();
  });
});

test.describe('Synonyms Page — With Data', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page);
    await page.goto('/index/test-index/synonyms');
  });

  test('displays synonym count badge', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();
    const countBadge = page.getByTestId('synonym-count');
    await expect(countBadge).toBeVisible();
    await expect(countBadge).toHaveText('4');
  });

  test('renders all synonym types with correct badges', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    await expect(list.getByText('Multi-way')).toBeVisible();
    await expect(list.getByText('One-way')).toBeVisible();
    await expect(list.getByText('Alt. Correction 1')).toBeVisible();
    await expect(list.getByText('Placeholder')).toBeVisible();
  });

  test('renders multi-way synonym description correctly', async ({ page }) => {
    await expect(page.getByText('hoodie = sweatshirt = pullover')).toBeVisible();
  });

  test('renders one-way synonym with arrow notation', async ({ page }) => {
    await expect(page.getByText(/phone → iphone, smartphone/)).toBeVisible();
  });

  test('renders placeholder synonym with braces', async ({ page }) => {
    await expect(page.getByText(/\{brand\} → nike, adidas/)).toBeVisible();
  });

  test('shows Edit and Delete buttons on each synonym row', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();
    const editButtons = list.getByRole('button', { name: /edit/i });
    await expect(editButtons).toHaveCount(4);
    const deleteButtons = list.getByRole('button', { name: /delete/i });
    await expect(deleteButtons).toHaveCount(4);
  });

  test('shows Clear All button when synonyms exist', async ({ page }) => {
    await expect(page.getByRole('button', { name: /clear all/i })).toBeVisible();
  });
});

test.describe('Synonyms Page — Header & Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page, EMPTY_SYNONYMS);
    await page.goto('/index/test-index/synonyms');
  });

  test('shows Synonyms heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Synonyms', exact: true })).toBeVisible();
  });

  test('shows search input', async ({ page }) => {
    await expect(page.getByPlaceholder(/search synonyms/i)).toBeVisible();
  });

  test('shows Add Synonym button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /add synonym/i })).toBeVisible();
  });

  test('shows breadcrumb back to index', async ({ page }) => {
    await expect(page.getByRole('button', { name: /test-index/i })).toBeVisible();
  });

  test('navigates back to search page via breadcrumb', async ({ page }) => {
    await page.getByRole('button', { name: /test-index/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index$/);
  });
});

test.describe('Synonyms Page — Create Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page, EMPTY_SYNONYMS);
    await mockSynonymSave(page);
    await page.goto('/index/test-index/synonyms');
  });

  test('opens create dialog with correct title', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('heading', { name: /create synonym/i })).toBeVisible();
  });

  test('shows all 5 type buttons in create dialog', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('button', { name: /multi-way/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /one-way/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /alt\. correction 1/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /alt\. correction 2/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /placeholder/i })).toBeVisible();
  });

  test('defaults to multi-way with two word inputs', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText(/words.*bidirectional/i)).toBeVisible();
    await expect(dialog.getByPlaceholder('Word 1')).toBeVisible();
    await expect(dialog.getByPlaceholder('Word 2')).toBeVisible();
  });

  test('switches to one-way type and shows input field', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /one-way/i }).click();
    await expect(dialog.getByText(/input.*source word/i)).toBeVisible();
    await expect(dialog.getByPlaceholder(/e\.g\. phone/i)).toBeVisible();
  });

  test('switches to placeholder type and shows token field', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /placeholder/i }).click();
    await expect(dialog.getByText(/placeholder token/i)).toBeVisible();
    await expect(dialog.getByPlaceholder(/e\.g\. brand_name/i)).toBeVisible();
  });

  test('Create button is disabled when form fields are empty', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    const createBtn = dialog.getByRole('button', { name: /^create$/i });
    await expect(createBtn).toBeDisabled();
  });

  test('Create button enables when multi-way form is valid', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');

    await dialog.getByPlaceholder('Word 1').fill('hoodie');
    await dialog.getByPlaceholder('Word 2').fill('sweatshirt');

    const createBtn = dialog.getByRole('button', { name: /^create$/i });
    await expect(createBtn).toBeEnabled();
  });

  test('can add more word inputs with Add Word button', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');

    await dialog.getByRole('button', { name: /add word/i }).click();
    await expect(dialog.getByPlaceholder('Word 3')).toBeVisible();
  });

  test('closes dialog on Cancel', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();

    await page.getByRole('button', { name: /cancel/i }).click();
    await expect(page.getByRole('dialog')).not.toBeVisible();
  });
});

test.describe('Synonyms Page — Edit Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page);
    await mockSynonymSave(page);
    await page.goto('/index/test-index/synonyms');
  });

  test('opens edit dialog with Edit Synonym title', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    await list.getByRole('button', { name: /edit/i }).first().click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('heading', { name: /edit synonym/i })).toBeVisible();
  });

  test('edit dialog has ID field disabled', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    await list.getByRole('button', { name: /edit/i }).first().click();
    const dialog = page.getByRole('dialog');
    const idInput = dialog.locator('input[disabled]').first();
    await expect(idInput).toBeVisible();
  });

  test('edit dialog does not show type selector buttons', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    await list.getByRole('button', { name: /edit/i }).first().click();
    const dialog = page.getByRole('dialog');
    // Type selector only appears when isCreating=true
    await expect(dialog.getByRole('button', { name: /one-way/i })).not.toBeVisible();
  });
});

test.describe('Synonyms Page — Delete & Clear Flows', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page);
    await mockSynonymSave(page);
    await page.goto('/index/test-index/synonyms');
  });

  test('delete synonym sends DELETE request after confirm', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    let deletedUrl = '';
    page.on('dialog', (dialog) => dialog.accept());
    page.on('request', (req) => {
      if (req.method() === 'DELETE' && req.url().includes('/synonyms/')) {
        deletedUrl = req.url();
      }
    });

    await list.getByRole('button', { name: /delete/i }).first().click();
    await expect.poll(() => deletedUrl).toContain('/synonyms/syn-1');
  });

  test('dismiss delete confirm does not send DELETE', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    let deleteRequested = false;
    page.on('dialog', (dialog) => dialog.dismiss());
    page.on('request', (req) => {
      if (req.method() === 'DELETE' && req.url().includes('/synonyms/')) {
        deleteRequested = true;
      }
    });

    await list.getByRole('button', { name: /delete/i }).first().click();
    // After dialog dismiss, verify the list is still visible (UI settled)
    await expect(list).toBeVisible();
    expect(deleteRequested).toBe(false);
  });

  test('Clear All sends clear request after confirm', async ({ page }) => {
    let clearRequested = false;
    await page.route('**/1/indexes/*/synonyms/clear', (route) => {
      clearRequested = true;
      route.fulfill({ status: 200, contentType: 'application/json', body: '{"updatedAt":"2026-02-09T00:00:00Z"}' });
    });

    page.on('dialog', (dialog) => dialog.accept());
    await page.getByRole('button', { name: /clear all/i }).click();
    await expect.poll(() => clearRequested).toBe(true);
  });
});

test.describe('Synonyms Page — Save/Create E2E', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page, EMPTY_SYNONYMS);
    await mockSynonymSave(page);
    await page.goto('/index/test-index/synonyms');
  });

  test('creating a multi-way synonym sends PUT with correct body', async ({ page }) => {
    let savedBody: any = null;
    page.on('request', (req) => {
      if (req.method() === 'PUT' && req.url().includes('/synonyms/')) {
        savedBody = req.postDataJSON();
      }
    });

    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');

    await dialog.getByPlaceholder('Word 1').fill('laptop');
    await dialog.getByPlaceholder('Word 2').fill('notebook');
    await dialog.getByRole('button', { name: /^create$/i }).click();

    await expect.poll(() => savedBody).toBeTruthy();
    expect(savedBody.type).toBe('synonym');
    expect(savedBody.synonyms).toContain('laptop');
    expect(savedBody.synonyms).toContain('notebook');
  });

  test('creating a one-way synonym sends correct body', async ({ page }) => {
    let savedBody: any = null;
    page.on('request', (req) => {
      if (req.method() === 'PUT' && req.url().includes('/synonyms/')) {
        savedBody = req.postDataJSON();
      }
    });

    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /one-way/i }).click();

    await dialog.getByPlaceholder(/e\.g\. phone/i).fill('phone');
    await dialog.getByPlaceholder('Synonym 1').fill('iphone');
    await dialog.getByRole('button', { name: /^create$/i }).click();

    await expect.poll(() => savedBody).toBeTruthy();
    expect(savedBody.type).toBe('onewaysynonym');
    expect(savedBody.input).toBe('phone');
    expect(savedBody.synonyms).toContain('iphone');
  });
});

test.describe('Synonyms Page — Edit Pre-population', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page);
    await mockSynonymSave(page);
    await page.goto('/index/test-index/synonyms');
  });

  test('edit dialog pre-populates multi-way synonym words', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    // Click Edit on first synonym (syn-1: hoodie, sweatshirt, pullover)
    await list.getByRole('button', { name: /edit/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Verify all 3 words are pre-filled
    await expect(dialog.locator('input[value="hoodie"]')).toBeVisible();
    await expect(dialog.locator('input[value="sweatshirt"]')).toBeVisible();
    await expect(dialog.locator('input[value="pullover"]')).toBeVisible();
  });

  test('edit dialog pre-populates objectID as disabled', async ({ page }) => {
    const list = page.locator('[data-testid="synonyms-list"]');
    await expect(list).toBeVisible();

    await list.getByRole('button', { name: /edit/i }).first().click();
    const dialog = page.getByRole('dialog');
    const idInput = dialog.locator('input[value="syn-1"]');
    await expect(idInput).toBeVisible();
    await expect(idInput).toBeDisabled();
  });
});

test.describe('Synonyms Page — Alt Correction Type Fields', () => {
  test.beforeEach(async ({ page }) => {
    await mockSynonymsApi(page, EMPTY_SYNONYMS);
    await page.goto('/index/test-index/synonyms');
  });

  test('alt correction 1 type shows Word and Corrections fields', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /alt\. correction 1/i }).click();

    await expect(dialog.getByText(/^word$/i)).toBeVisible();
    await expect(dialog.getByPlaceholder(/e\.g\. smartphone/i)).toBeVisible();
    await expect(dialog.getByText(/^corrections$/i)).toBeVisible();
    await expect(dialog.getByPlaceholder('Correction 1')).toBeVisible();
  });

  test('alt correction 2 type shows same layout as alt correction 1', async ({ page }) => {
    await page.getByRole('button', { name: /add synonym/i }).click();
    const dialog = page.getByRole('dialog');
    await dialog.getByRole('button', { name: /alt\. correction 2/i }).click();

    await expect(dialog.getByPlaceholder(/e\.g\. smartphone/i)).toBeVisible();
    await expect(dialog.getByPlaceholder('Correction 1')).toBeVisible();
  });
});

test.describe('Synonyms Page — Loading State', () => {
  test('shows skeleton cards while loading', async ({ page }) => {
    // Delay the API response to observe loading state
    await page.route('**/1/indexes/*/synonyms/search', async (route) => {
      await new Promise((r) => setTimeout(r, 2000));
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_SYNONYMS),
      });
    });

    await page.goto('/index/test-index/synonyms');
    // Skeletons should be visible during load
    const skeletons = page.locator('.animate-pulse');
    await expect(skeletons.first()).toBeVisible();
  });
});

test.describe('Synonyms Page — Search/Filter', () => {
  test('sends search query to the synonyms search API', async ({ page }) => {
    let lastBody: any = null;
    await page.route('**/1/indexes/*/synonyms/search', (route) => {
      lastBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(EMPTY_SYNONYMS),
      });
    });

    await page.goto('/index/test-index/synonyms');
    // Wait for initial load
    await expect(page.getByText(/no synonyms/i)).toBeVisible();

    await page.getByPlaceholder(/search synonyms/i).fill('hoodie');

    // Wait for the debounced API call with the new query
    await page.waitForResponse(
      (response) => response.url().includes('/synonyms/search') && response.status() === 200
    );

    expect(lastBody?.query).toBe('hoodie');
  });
});
