# WordPress Plugin E2E Tests

Comprehensive end-to-end tests for the Flapjack Search WordPress plugin using Playwright.

## Overview

This test suite covers **35 user-facing behaviors** across 7 test files:

| Test File | Behaviors | Description |
|-----------|-----------|-------------|
| `01-instantsearch-overlay.spec.js` | 4 | InstantSearch overlay UI interactions |
| `02-woocommerce-facets.spec.js` | 8 | WooCommerce faceted search functionality |
| `03-settings-page.spec.js` | 10 | Admin settings page UI and validation |
| `04-gutenberg-block.spec.js` | 3 | Gutenberg block editor integration |
| `05-backend-search.spec.js` | 3 | WordPress admin backend search |
| `06-autocomplete.spec.js` | 3 | Search autocomplete dropdown |
| `07-activation-setup.spec.js` | 4 | Plugin activation and setup flows |

**Total:** 35 E2E tests

## Setup

### Prerequisites

- Node.js 18+ and npm
- Docker Desktop (for local WordPress environment via @wordpress/env)
- OR a live WordPress site URL for testing

### Installation

```bash
# Install dependencies
npm install

# Install Playwright browsers
npx playwright install chromium

# (Optional) Install all browsers for cross-browser testing
npx playwright install
```

### Environment Configuration

Copy `.env.example` to `.env` and configure:

```bash
cp .env.example .env
```

Edit `.env` with your values:

```env
# Local testing (default)
WP_BASE_URL=http://localhost:8888
WP_ADMIN_USER=admin
WP_ADMIN_PASSWORD=password
TEST_MODE=local

# Production testing
# WP_BASE_URL=https://your-wordpress-site.com
# WP_ADMIN_USER=your_admin
# WP_ADMIN_PASSWORD=your_password
# TEST_MODE=production
# FLAPJACK_APP_ID=your_real_app_id
# FLAPJACK_ADMIN_API_KEY=your_real_admin_key
# FLAPJACK_SEARCH_API_KEY=your_real_search_key
```

## Running Tests

### Option 1: Local WordPress Environment (@wordpress/env)

```bash
# Start local WordPress with Docker
npm run wp-env:start

# Wait for WordPress to be ready (first time takes ~2 minutes)
# WordPress will be available at: http://localhost:8888
# Admin: http://localhost:8888/wp-admin (admin / password)

# Run E2E tests
npm run test:e2e

# Stop environment when done
npm run wp-env:stop
```

### Option 2: Production/Staging Site

```bash
# Configure .env with production URL and credentials
# Set TEST_MODE=production
# Add real Flapjack API credentials

# Run tests against production
npm run test:e2e
```

### Test Commands

```bash
# Run all E2E tests (headless)
npm run test:e2e

# Run with UI mode (interactive)
npm run test:e2e:ui

# Run in headed mode (see browser)
npm run test:e2e:headed

# Debug specific test
npm run test:e2e:debug

# Run specific test file
npx playwright test 01-instantsearch-overlay.spec.js

# Run specific test by name
npx playwright test -g "BEH-IS-001"

# View test report
npm run test:e2e:report
```

## Test Modes

### Local Mode (TEST_MODE=local)

- Uses mocked Flapjack API responses
- Faster test execution
- No real API calls
- Ideal for development and CI/CD

### Production Mode (TEST_MODE=production)

- Uses real Flapjack API
- Tests actual search functionality
- Requires valid API credentials
- Slower but more realistic

## Test Behavior Specification

All tests are based on the behavior specification document:
`docs2/integrations/tests/WORDPRESS_E2E_BEHAVIOR_SPEC.md`

Each test is labeled with its behavior ID (e.g., `BEH-IS-001`) for traceability.

## Test Results

Results are saved in:
- `test-results/` - Screenshots, videos, traces
- `playwright-report/` - HTML report
- `test-results/e2e-results.json` - JSON report

## Continuous Integration

Add to `.github/workflows/e2e-tests.yml`:

```yaml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'

      - name: Install dependencies
        run: npm install

      - name: Install Playwright
        run: npx playwright install --with-deps chromium

      - name: Start WordPress
        run: npm run wp-env:start

      - name: Run E2E tests
        run: npm run test:e2e

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: playwright-report
          path: playwright-report/
```

## Troubleshooting

### Tests fail with "Cannot find element"

- Ensure WordPress is fully loaded before tests run
- Check selectors match your plugin's HTML structure
- Some tests may need selector adjustments for your specific implementation

### wp-env fails to start

```bash
# Check Docker is running
docker ps

# Reset environment
npm run wp-env:destroy
npm run wp-env:start
```

### Tests timeout

- Increase timeout in `playwright.config.js`
- Check network connectivity
- Verify WordPress/Flapjack API is responding

### Authentication fails

- Verify credentials in `.env`
- Check WordPress user has admin privileges
- Clear WordPress cookies: `await context.clearCookies()`

## Writing New Tests

1. Create new `.spec.js` file in `tests/e2e/`
2. Import helpers from `./helpers.js`
3. Use behavior-driven structure:
   ```javascript
   test('BEH-XX-001: Description of behavior', async ({ page }) => {
     // Preconditions
     // Actions
     // Expected outcomes with ✅ comments
   });
   ```
4. Add to behavior spec document
5. Run and verify

## Test Helpers

See `helpers.js` for reusable functions:

- `loginToWordPress(page)` - Admin login
- `navigateToSettings(page)` - Go to plugin settings
- `saveAPICredentials(page, creds)` - Save API keys
- `activatePlugin(page)` - Activate plugin
- `deactivatePlugin(page)` - Deactivate plugin
- `mockFlapjackAPI(page)` - Mock API for local testing
- `triggerReindex(page)` - Start reindexing

## Coverage

Current coverage: **35/35 behaviors (100%)**

- ✅ InstantSearch overlay: 4/4
- ✅ WooCommerce facets: 8/8
- ✅ Settings page: 10/10
- ✅ Gutenberg blocks: 3/3
- ✅ Backend search: 3/3
- ✅ Autocomplete: 3/3
- ✅ Activation flows: 4/4

## Next Steps

- [ ] Run tests locally with wp-env
- [ ] Run tests against staging site
- [ ] Add to CI/CD pipeline
- [ ] Add visual regression tests
- [ ] Add accessibility tests
- [ ] Add performance tests

## Resources

- [Playwright Documentation](https://playwright.dev)
- [WordPress E2E Testing](https://developer.wordpress.org/block-editor/reference-guides/packages/packages-e2e-test-utils/)
- [@wordpress/env Guide](https://developer.wordpress.org/block-editor/reference-guides/packages/packages-env/)
- [Behavior Spec](../../docs2/integrations/tests/WORDPRESS_E2E_BEHAVIOR_SPEC.md)
