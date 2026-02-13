# Flapjack Dashboard — E2E Tests

End-to-end tests for the Flapjack dashboard using [Playwright](https://playwright.dev).

## Directory Structure

```
tests/
  global-setup.ts              Loads .env.secret for integration tests
  fixtures/
    auth.fixture.ts            Shared localStorage auth seeding
    algolia.fixture.ts         Algolia index seed / teardown helpers
    test-data.ts               Test data constants (products, synonyms, rules)
  pages/                       UI-only page tests (no external services)
    overview.spec.ts
    search.spec.ts
    settings.spec.ts
    apikeys.spec.ts
    navigation.spec.ts
    system.spec.ts
    migrate.spec.ts            Migrate page UI (form, validation, toggles)
  integration/                 Tests requiring external services (Algolia)
    migrate.spec.ts            Full migration E2E (seeds Algolia → migrates → verifies)
```

**pages/** — Fast tests that only need a running Flapjack server + Vite dev server.

**integration/** — Tests that talk to external services (Algolia). They skip gracefully when credentials are not configured.

## Prerequisites

- **Flapjack server** running on `localhost:7700`
- **Vite dev server** on `localhost:5177` (started automatically by Playwright unless already running)
- **Algolia credentials** (integration tests only) — set `ALGOLIA_APP_ID` and `ALGOLIA_ADMIN_KEY` in `../../.secret/.env.secret`

## Running Tests

```bash
# All tests (page + integration)
npm test

# Only page tests (fast, no Algolia credentials needed)
npm run test:pages

# Only integration tests (requires Algolia credentials)
npm run test:integration

# Interactive UI mode (recommended for development)
npm run test:ui

# Headed mode (see the browser)
npm run test:headed

# Debug mode (step through with Playwright Inspector)
npm run test:debug

# View the last HTML report
npm run test:report
```

## No Sleeps Policy

**STRICT RULE: Never use `page.waitForTimeout()`, `setTimeout`, or any hardcoded delay in tests.** Every wait must target a specific condition — an element appearing, an API response arriving, or a value changing. Tests that sleep are slow, flaky, and hide real bugs.

| Instead of | Use |
|---|---|
| `page.waitForTimeout(2000)` | `expect(locator).toBeVisible()` |
| Sleeping after a click | `page.waitForResponse(predicate)` |
| Arbitrary delay for data | `expect(locator).toHaveText(expected)` |
| Waiting for async state change | `expect(async () => { ... }).toPass()` |

All Playwright `expect()` assertions auto-retry until the condition is met or the timeout expires. Pass `{ timeout: N }` for longer waits (e.g. migration completion).

### `expect().toPass()` for complex polling

When you need to poll for a condition that involves reading a value and asserting on it (not just a simple locator check), use `expect().toPass()`:

```typescript
// Poll until filtered data appears after selecting a filter
await expect(async () => {
  const text = await page.locator('.count').textContent();
  const num = parseInt(text!.replace(/,/g, ''), 10);
  expect(num).toBeLessThan(previousValue);
}).toPass({ timeout: 10000 });
```

### `waitForResponse` for API-triggered updates

When a user action triggers an API call and you need the response before asserting:

```typescript
const responsePromise = page.waitForResponse(resp => resp.url().includes('/2/searches'));
await page.getByTestId('range-30d').click();
await responsePromise;
```

### `.or()` for unknown server state

When the server state is unknown (e.g. indices may or may not exist), use Playwright's `.or()` pattern to wait for one of several valid outcomes:

```typescript
await expect(
  page.getByText('Search Behavior').or(page.getByText(/no indices/i)),
).toBeVisible();
```

### Node-level polling (fixtures, setup)

In non-browser code (fixtures, setup scripts), a manual poll loop with a backoff delay is acceptable since Playwright's `expect()` isn't available:

```typescript
while (Date.now() - start < maxWaitMs) {
  const result = await client.search({ ... });
  if (result.nbHits >= expected) return;
  await new Promise(r => setTimeout(r, 500)); // backoff between retries — OK
}
```

This is the only context where `setTimeout` is acceptable — as a retry backoff inside a polling loop, not as a standalone sleep.

## Writing New Tests

### Using the auth fixture

**All specs** import `test` and `expect` from the auth fixture so every test gets an authenticated page automatically. Do not import from `@playwright/test` directly:

```typescript
import { test, expect } from '../fixtures/auth.fixture';

test('my test', async ({ page }) => {
  await page.goto('/overview');
  // localStorage is already seeded with API key
});
```

### Adding integration tests

Place tests that require external services in `tests/integration/`. Use the conditional skip pattern:

```typescript
import { test, expect } from '../fixtures/auth.fixture';
import { hasAlgoliaCredentials } from '../fixtures/algolia.fixture';

const describeOrSkip = hasAlgoliaCredentials()
  ? test.describe
  : test.describe.skip;

describeOrSkip('My Integration Test', () => {
  // Tests here skip gracefully when credentials are missing
});
```

## Debugging

```bash
# UI mode — real-time test execution with time-travel debugging
npm run test:ui

# Debug mode — step through line by line with Playwright Inspector
npm run test:debug

# Take a screenshot mid-test
await page.screenshot({ path: 'debug.png' });

# Pause execution and open Inspector
await page.pause();
```

## Common Issues

### Tests fail with "No API response"
The dashboard needs a running Flapjack server at `http://localhost:7700`.

### Integration tests are skipped
Set `ALGOLIA_APP_ID` and `ALGOLIA_ADMIN_KEY` in `.secret/.env.secret` at the project root.

### Tests timeout
Increase timeout in `playwright.config.ts` or per-test with `test.describe.configure({ timeout: 120_000 })`.

## Browser Setup

Only Chromium is configured by default. To test other browsers:

```bash
npx playwright install firefox webkit
```

Then uncomment the browser configs in `playwright.config.ts`.

## Test Coverage

### Page Tests
- **Overview** — stats cards, index list, create index dialog (validation, templates, cancel), dark mode
- **Search & Browse** — search input, results panel, facets, add documents dialog (JSON/upload/sample tabs), breadcrumb navigation
- **Settings** — settings form, searchable attributes, faceting, save button, no-indices empty state
- **API Keys** — create key dialog, form fields, permissions, cancel, page description
- **Navigation** — sidebar links, active state highlighting, dark mode persistence, 404 handling, connection status, logo link, migrate link, connection settings dialog
- **System** — health/indices/replication tabs, sidebar navigation
- **Migrate (UI)** — form inputs, API key visibility toggle, button enable/disable, dynamic button text, target placeholder, overwrite toggle, info section

### Integration Tests
- **Migrate** — full Algolia-to-Flapjack migration: seeds Algolia with 12 products + synonyms + rules, fills the migration form in the browser, verifies success card with correct counts, navigates to the index, confirms documents are searchable. Also tests error state with invalid credentials.
