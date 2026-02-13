/**
 * E2E Test Helpers for WordPress Plugin
 */

/**
 * Login to WordPress admin
 * @param {import('@playwright/test').Page} page
 * @param {string} username
 * @param {string} password
 */
export async function loginToWordPress(page, username = 'admin', password = 'password') {
  // Check if already logged in
  await page.goto('/wp-admin/');

  const isLoggedIn = await page.locator('body.wp-admin').count() > 0;
  if (isLoggedIn) {
    return;
  }

  // Fill login form
  await page.fill('#user_login', username);
  await page.fill('#user_pass', password);
  await page.click('#wp-submit');

  // Wait for admin dashboard
  await page.waitForURL('**/wp-admin/**');
}

/**
 * Navigate to Flapjack settings page
 * @param {import('@playwright/test').Page} page
 */
export async function navigateToSettings(page) {
  await page.goto('/wp-admin/admin.php?page=flapjack-search');
  await page.waitForLoadState('networkidle');
}

/**
 * Save Flapjack API credentials
 * @param {import('@playwright/test').Page} page
 * @param {object} credentials
 */
export async function saveAPICredentials(page, credentials) {
  await navigateToSettings(page);

  // Fill API credentials
  if (credentials.appId) {
    await page.fill('#flapjack_app_id', credentials.appId);
  }
  if (credentials.adminApiKey) {
    await page.fill('#flapjack_admin_api_key', credentials.adminApiKey);
  }
  if (credentials.searchApiKey) {
    await page.fill('#flapjack_search_api_key', credentials.searchApiKey);
  }

  // Save settings
  await page.click('button[type="submit"]');

  // Wait for save confirmation
  await page.waitForSelector('.notice-success, .notice-error');
}

/**
 * Activate plugin if not already active
 * @param {import('@playwright/test').Page} page
 */
export async function activatePlugin(page) {
  await page.goto('/wp-admin/plugins.php');

  // Check if already active
  const isActive = await page.locator('[data-slug="flapjack-search"] .deactivate').count() > 0;
  if (isActive) {
    return;
  }

  // Find and click activate link
  const activateLink = page.locator('[data-slug="flapjack-search"] .activate a');
  if (await activateLink.count() > 0) {
    await activateLink.click();
    await page.waitForURL('**/plugins.php**');
  }
}

/**
 * Deactivate plugin if active
 * @param {import('@playwright/test').Page} page
 */
export async function deactivatePlugin(page) {
  await page.goto('/wp-admin/plugins.php');

  const deactivateLink = page.locator('[data-slug="flapjack-search"] .deactivate a');
  if (await deactivateLink.count() > 0) {
    await deactivateLink.click();
    await page.waitForURL('**/plugins.php**');
  }
}

/**
 * Create test posts/products
 * @param {import('@playwright/test').Page} page
 * @param {number} count
 * @param {string} type - 'post' or 'product'
 */
export async function createTestContent(page, count = 10, type = 'post') {
  const contentType = type === 'product' ? 'product' : 'post';

  for (let i = 1; i <= count; i++) {
    await page.goto(`/wp-admin/${contentType === 'product' ? 'post-new.php?post_type=product' : 'post-new.php'}`);

    // Fill title
    await page.fill('#title', `Test ${contentType} ${i}`);

    // Fill content (in block editor)
    const editorSelector = '.block-editor-writing-flow';
    if (await page.locator(editorSelector).count() > 0) {
      await page.click(editorSelector);
      await page.keyboard.type(`This is test ${contentType} ${i} content.`);
    }

    // Publish
    await page.click('button.editor-post-publish-button__button');
    if (await page.locator('button.editor-post-publish-button').count() > 0) {
      await page.click('button.editor-post-publish-button');
    }

    // Wait for publish confirmation
    await page.waitForSelector('.components-snackbar', { timeout: 5000 }).catch(() => {});
  }
}

/**
 * Trigger reindex
 * @param {import('@playwright/test').Page} page
 */
export async function triggerReindex(page) {
  await navigateToSettings(page);

  // Click reindex button
  await page.click('#flapjack_reindex_button');

  // Wait for progress or completion
  await page.waitForSelector('.flapjack-reindex-progress, .notice-success', { timeout: 60000 });
}

/**
 * Mock Flapjack API responses
 * @param {import('@playwright/test').Page} page
 */
export async function mockFlapjackAPI(page) {
  // Mock search API
  await page.route('**/1/indexes/*/query', async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        hits: [
          { objectID: '1', title: 'Test Result 1', excerpt: 'Test excerpt 1' },
          { objectID: '2', title: 'Test Result 2', excerpt: 'Test excerpt 2' },
        ],
        nbHits: 2,
        page: 0,
        nbPages: 1,
        hitsPerPage: 20,
      }),
    });
  });

  // Mock batch search
  await page.route('**/1/indexes/*/queries', async route => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        results: [
          {
            hits: [
              { objectID: '1', title: 'Test Result 1' },
            ],
            nbHits: 1,
          },
        ],
      }),
    });
  });
}

/**
 * Wait for element and check visibility
 * @param {import('@playwright/test').Page} page
 * @param {string} selector
 * @param {number} timeout
 */
export async function waitForVisible(page, selector, timeout = 5000) {
  await page.waitForSelector(selector, { state: 'visible', timeout });
}

/**
 * Get environment variable or throw error
 * @param {string} name
 * @param {string} defaultValue
 */
export function getEnv(name, defaultValue = null) {
  const value = process.env[name] || defaultValue;
  if (value === null) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

/**
 * Get test credentials from environment
 */
export function getTestCredentials() {
  return {
    baseURL: getEnv('WP_BASE_URL', 'http://localhost:8888'),
    adminUser: getEnv('WP_ADMIN_USER', 'admin'),
    adminPassword: getEnv('WP_ADMIN_PASSWORD', 'password'),
    flapjackAppId: getEnv('FLAPJACK_APP_ID', 'test_app_id'),
    flapjackAdminApiKey: getEnv('FLAPJACK_ADMIN_API_KEY', 'test_admin_key'),
    flapjackSearchApiKey: getEnv('FLAPJACK_SEARCH_API_KEY', 'test_search_key'),
  };
}
