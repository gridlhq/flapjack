import { test, expect } from '../fixtures/auth.fixture';
import type { Page } from '@playwright/test';

const MOCK_RULES = {
  hits: [
    {
      objectID: 'rule-promo-iphone',
      conditions: [{ pattern: 'iphone', anchoring: 'is' }],
      consequence: {
        promote: [{ objectID: 'prod-123', position: 0 }],
        hide: [{ objectID: 'prod-999' }],
      },
      description: 'Promote iPhone 15 for "iphone" query',
      enabled: true,
    },
    {
      objectID: 'rule-sale-banner',
      conditions: [{ pattern: 'sale', anchoring: 'contains' }],
      consequence: {
        params: { query: 'deals' },
      },
      description: 'Rewrite "sale" queries to "deals"',
      enabled: false,
    },
  ],
  nbHits: 2,
  page: 0,
  nbPages: 1,
};

const EMPTY_RULES = { hits: [], nbHits: 0, page: 0, nbPages: 0 };

function mockRulesApi(page: Page, response = MOCK_RULES) {
  return page.route('**/1/indexes/*/rules/search', (route) => {
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(response),
    });
  });
}

function mockRuleSave(page: Page) {
  return page.route('**/1/indexes/*/rules/*', (route) => {
    // Don't intercept the /rules/search endpoint — let the search mock handle it
    if (route.request().url().includes('/rules/search')) {
      return route.fallback();
    }
    if (route.request().method() === 'PUT') {
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ objectID: 'rule-new', updatedAt: new Date().toISOString() }),
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

test.describe('Rules Page — Empty State', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page, EMPTY_RULES);
    await page.goto('/index/test-index/rules');
  });

  test('shows empty state message when no rules exist', async ({ page }) => {
    await expect(page.getByText(/no rules/i)).toBeVisible();
    await expect(page.getByText(/rules let you customize search results/i)).toBeVisible();
  });

  test('shows Create a Rule button in empty state', async ({ page }) => {
    await expect(page.getByText(/no rules/i)).toBeVisible();
    await expect(page.getByRole('button', { name: /create a rule/i })).toBeVisible();
  });

  test('shows Open Merchandising Studio link in empty state', async ({ page }) => {
    await expect(page.getByText(/no rules/i)).toBeVisible();
    await expect(page.getByRole('link', { name: /open merchandising studio/i })).toBeVisible();
  });

  test('does not show Clear All button when no rules exist', async ({ page }) => {
    await expect(page.getByText(/no rules/i)).toBeVisible();
    await expect(page.getByRole('button', { name: /clear all/i })).not.toBeVisible();
  });
});

test.describe('Rules Page — With Data', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page);
    await page.goto('/index/test-index/rules');
  });

  test('renders correct number of rule items', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();
    const cards = list.getByTestId('rule-card');
    await expect(cards).toHaveCount(2);
  });

  test('renders rule objectIDs', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    await expect(list.getByText('rule-promo-iphone')).toBeVisible();
    await expect(list.getByText('rule-sale-banner')).toBeVisible();
  });

  test('shows pinned and hidden count badges', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    await expect(list.getByText('1 pinned')).toBeVisible();
    await expect(list.getByText('1 hidden')).toBeVisible();
  });

  test('shows rule descriptions', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    await expect(list.getByText(/promote iphone 15/i)).toBeVisible();
    await expect(list.getByText(/rewrite.*sale.*queries/i)).toBeVisible();
  });

  test('shows Edit and Delete buttons on each rule row', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();
    const editButtons = list.getByRole('button', { name: /edit/i });
    await expect(editButtons).toHaveCount(2);
    const deleteButtons = list.getByRole('button', { name: /delete/i });
    await expect(deleteButtons).toHaveCount(2);
  });

  test('shows Clear All button when rules exist', async ({ page }) => {
    await expect(page.getByRole('button', { name: /clear all/i })).toBeVisible();
  });
});

test.describe('Rules Page — Header & Navigation', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page, EMPTY_RULES);
    await page.goto('/index/test-index/rules');
  });

  test('shows Rules heading', async ({ page }) => {
    await expect(page.getByRole('heading', { name: 'Rules', exact: true })).toBeVisible();
  });

  test('shows search input', async ({ page }) => {
    await expect(page.getByPlaceholder(/search rules/i)).toBeVisible();
  });

  test('shows Add Rule button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /add rule/i })).toBeVisible();
  });

  test('shows Merchandising Studio link in header', async ({ page }) => {
    await expect(page.getByRole('link', { name: /merchandising studio/i }).first()).toBeVisible();
  });

  test('shows breadcrumb back to index', async ({ page }) => {
    await expect(page.getByRole('button', { name: /test-index/i })).toBeVisible();
  });

  test('navigates to merchandising studio', async ({ page }) => {
    await page.getByRole('link', { name: /merchandising studio/i }).first().click();
    await expect(page).toHaveURL(/\/index\/test-index\/merchandising/);
  });

  test('navigates back to search page via breadcrumb', async ({ page }) => {
    await page.getByRole('button', { name: /test-index/i }).click();
    await expect(page).toHaveURL(/\/index\/test-index$/);
  });
});

test.describe('Rules Page — Create Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page, EMPTY_RULES);
    await mockRuleSave(page);
    await page.goto('/index/test-index/rules');
  });

  test('opens create rule dialog with JSON editor', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(page.getByRole('heading', { name: /create rule/i })).toBeVisible();
    // Verify the JSON editor area is present (border container wrapping the lazy-loaded editor)
    await expect(dialog.locator('.border.rounded-md.overflow-hidden')).toBeVisible();
  });

  test('create dialog shows helpful description', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByText(/edit the rule json directly/i)).toBeVisible();
  });

  test('create dialog has Create and Cancel buttons', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('button', { name: /^create$/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /cancel/i })).toBeVisible();
  });

  test('closes dialog on Cancel', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    await expect(page.getByRole('dialog')).toBeVisible();

    await page.getByRole('button', { name: /cancel/i }).click();
    await expect(page.getByRole('dialog')).not.toBeVisible();
  });
});

test.describe('Rules Page — Edit Dialog', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page);
    await mockRuleSave(page);
    await page.goto('/index/test-index/rules');
  });

  test('opens edit dialog with rule objectID in title', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    await list.getByRole('button', { name: /edit/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(page.getByRole('heading', { name: /edit rule.*rule-promo-iphone/i })).toBeVisible();
  });

  test('edit dialog shows Save and Cancel buttons', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    await list.getByRole('button', { name: /edit/i }).first().click();
    const dialog = page.getByRole('dialog');
    await expect(dialog.getByRole('button', { name: /^save$/i })).toBeVisible();
    await expect(dialog.getByRole('button', { name: /cancel/i })).toBeVisible();
  });
});

test.describe('Rules Page — Enabled/Disabled Indicator', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page);
    await page.goto('/index/test-index/rules');
  });

  test('enabled rule shows green Power icon', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();
    // First rule (rule-promo-iphone) is enabled: should have green Power icon
    const firstCard = list.getByTestId('rule-card').first();
    await expect(firstCard.locator('.text-green-500')).toBeVisible();
  });

  test('disabled rule shows muted PowerOff icon', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();
    // Second rule (rule-sale-banner) is disabled: should have muted PowerOff
    const secondCard = list.getByTestId('rule-card').nth(1);
    await expect(secondCard.locator('.text-muted-foreground').first()).toBeVisible();
  });
});

test.describe('Rules Page — Delete & Clear Flows', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page);
    await mockRuleSave(page);
    await page.goto('/index/test-index/rules');
  });

  test('delete rule sends DELETE request after confirm', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    let deletedUrl = '';
    page.on('dialog', (dialog) => dialog.accept());
    page.on('request', (req) => {
      if (req.method() === 'DELETE' && req.url().includes('/rules/')) {
        deletedUrl = req.url();
      }
    });

    await list.getByRole('button', { name: /delete/i }).first().click();
    await expect.poll(() => deletedUrl).toContain('/rules/rule-promo-iphone');
  });

  test('dismiss delete confirm does not send DELETE', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();

    let deleteRequested = false;
    page.on('dialog', (dialog) => dialog.dismiss());
    page.on('request', (req) => {
      if (req.method() === 'DELETE' && req.url().includes('/rules/')) {
        deleteRequested = true;
      }
    });

    await list.getByRole('button', { name: /delete/i }).first().click();
    // Wait for the dialog dismiss to complete — then assert no DELETE was fired
    // The dismiss handler fires synchronously, and no async request should follow
    await expect(list).toBeVisible(); // ensures UI settled after dialog dismiss
    expect(deleteRequested).toBe(false);
  });

  test('Clear All sends clear request after confirm', async ({ page }) => {
    let clearRequested = false;
    await page.route('**/1/indexes/*/rules/clear', (route) => {
      clearRequested = true;
      route.fulfill({ status: 200, contentType: 'application/json', body: '{"updatedAt":"2026-02-09T00:00:00Z"}' });
    });

    page.on('dialog', (dialog) => dialog.accept());
    await page.getByRole('button', { name: /clear all/i }).click();
    await expect.poll(() => clearRequested).toBe(true);
  });
});

test.describe('Rules Page — Auto-Generated Description', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page);
    await page.goto('/index/test-index/rules');
  });

  test('shows generated description for rule with promote and hide', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();
    // rule-promo-iphone: When query is "iphone", pin 1 result, hide 1 result
    await expect(list.getByText(/when query is "iphone"/i)).toBeVisible();
    await expect(list.getByText(/pin 1 result/i)).toBeVisible();
    await expect(list.getByText(/hide 1 result/i)).toBeVisible();
  });

  test('shows generated description for rule with query rewrite', async ({ page }) => {
    const list = page.locator('[data-testid="rules-list"]');
    await expect(list).toBeVisible();
    // rule-sale-banner: When query contains "sale", modify query
    await expect(list.getByText(/when query contains "sale"/i)).toBeVisible();
    await expect(list.getByText(/modify query/i)).toBeVisible();
  });
});

test.describe('Rules Page — Rule Count Badge', () => {
  test('shows rule count in badge', async ({ page }) => {
    await mockRulesApi(page);
    await page.goto('/index/test-index/rules');
    const badge = page.getByTestId('rules-count-badge');
    await expect(badge).toBeVisible();
    await expect(badge).toHaveText('2');
  });
});

test.describe('Rules Page — Loading State', () => {
  test('shows skeleton cards while loading', async ({ page }) => {
    await page.route('**/1/indexes/*/rules/search', async (route) => {
      await new Promise((r) => setTimeout(r, 2000));
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(MOCK_RULES),
      });
    });

    await page.goto('/index/test-index/rules');
    const skeletons = page.locator('.animate-pulse');
    await expect(skeletons.first()).toBeVisible();
  });
});

test.describe('Rules Page — Create E2E', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page, EMPTY_RULES);
    await mockRuleSave(page);
    await page.goto('/index/test-index/rules');
  });

  test('creating a rule with default JSON sends PUT request', async ({ page }) => {
    let savedUrl = '';
    page.on('request', (req) => {
      if (req.method() === 'PUT' && req.url().includes('/rules/') && !req.url().includes('/rules/search')) {
        savedUrl = req.url();
      }
    });

    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Default JSON has valid objectID and consequence, so Create should work
    await dialog.getByRole('button', { name: /^create$/i }).click();
    await expect.poll(() => savedUrl).toContain('/rules/');
  });
});

test.describe('Rules Page — JSON Validation Errors', () => {
  test.beforeEach(async ({ page }) => {
    await mockRulesApi(page, EMPTY_RULES);
    await mockRuleSave(page);
    await page.goto('/index/test-index/rules');
  });

  test('invalid JSON shows parse error message', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Wait for Monaco editor to load
    await expect(dialog.locator('.monaco-editor')).toBeVisible();

    // Set invalid JSON via Monaco model API
    await page.evaluate(() => {
      const editor = (window as any).monaco?.editor?.getEditors?.()?.[0];
      if (editor) editor.getModel()?.setValue('{ invalid json !!!');
    });

    // Click Create — should trigger validation
    await dialog.getByRole('button', { name: /^create$/i }).click();

    // Error message should appear (parse error from JSON.parse)
    await expect(dialog.locator('.text-destructive')).toBeVisible();
  });

  test('JSON missing objectID shows "objectID is required" error', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.locator('.monaco-editor')).toBeVisible();

    // Wait for Monaco to fully initialize before setting content
    await page.waitForFunction(() => {
      const editors = (window as any).monaco?.editor?.getEditors?.();
      return editors?.length > 0;
    }, null, { timeout: 10000 });
    await page.evaluate(() => {
      (window as any).monaco.editor.getEditors()[0].getModel().setValue('{"consequence":{}}');
    });

    await dialog.getByRole('button', { name: /^create$/i }).click();
    await expect(dialog.getByText('objectID is required')).toBeVisible();
  });

  test('JSON missing consequence shows "consequence is required" error', async ({ page }) => {
    await page.getByRole('button', { name: /add rule/i }).click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.locator('.monaco-editor')).toBeVisible();

    // Wait for Monaco to fully initialize before setting content
    await page.waitForFunction(() => {
      const editors = (window as any).monaco?.editor?.getEditors?.();
      return editors?.length > 0;
    }, null, { timeout: 10000 });
    await page.evaluate(() => {
      (window as any).monaco.editor.getEditors()[0].getModel().setValue('{"objectID":"test-rule"}');
    });

    await dialog.getByRole('button', { name: /^create$/i }).click();
    await expect(dialog.getByText('consequence is required')).toBeVisible();
  });
});

test.describe('Rules Page — Search/Filter', () => {
  test('sends search query to the rules search API', async ({ page }) => {
    let lastBody: any = null;
    await page.route('**/1/indexes/*/rules/search', (route) => {
      lastBody = route.request().postDataJSON();
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(EMPTY_RULES),
      });
    });

    await page.goto('/index/test-index/rules');
    await expect(page.getByText(/no rules/i)).toBeVisible();

    await page.getByPlaceholder(/search rules/i).fill('iphone');

    await page.waitForResponse(
      (response) => response.url().includes('/rules/search') && response.status() === 200
    );

    expect(lastBody?.query).toBe('iphone');
  });
});
