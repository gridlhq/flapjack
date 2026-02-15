# Flapjack Dashboard — E2E Tests

End-to-end tests for the Flapjack dashboard using [Playwright](https://playwright.dev).

## Test Categories — IMPORTANT DISTINCTION

| Category | Directory | What it does | Browser? |
|----------|-----------|--------------|----------|
| **e2e-ui** | `tests/e2e-ui/` | Simulated-human interaction with a real rendered browser. Clicks, types, navigates exactly like a user would. **NO mocks.** | YES — real Chromium |
| **e2e-api** | `tests/e2e-api/` | REST API calls directly to the server. Verifies response shapes, data integrity, and API contracts. Some tests also open a browser for hybrid verification. | Mixed — mostly API |

When we say "e2e-ui tests" we mean **non-mocked, simulated-human, real-browser tests**. Not API tests. Not unit tests. Real browser.

## Directory Structure

```
tests/
  global-setup.ts              Loads .env.secret for e2e-api tests
  fixtures/
    auth.fixture.ts            Shared localStorage auth seeding
    algolia.fixture.ts         Algolia index seed / teardown helpers
    analytics-seed.ts          Analytics data seeding helpers
    test-data.ts               Test data constants (products, synonyms, rules)
  e2e-ui/                      REAL BROWSER — simulated human interaction
    seed.setup.ts              Seeds test data before e2e-ui suite
    cleanup.setup.ts           Tears down test data after e2e-ui suite
    helpers.ts                 Constants and re-exports
    smoke/                     Critical path smoke tests (~2 min)
      critical-paths.spec.ts   7 must-pass tests
    full/                      Comprehensive E2E-UI suite (10-15 min)
      overview.spec.ts         Index management, stat cards, create/delete
      search.spec.ts           Search, facets, filtering, pagination
      analytics.spec.ts        6 tabs, KPIs, charts, date ranges
      analytics-deep.spec.ts   Deep data verification (seeded values)
      rules.spec.ts            Rule CRUD, JSON editor, conditions
      synonyms.spec.ts         Synonym CRUD, types, search/filter
      settings.spec.ts         Settings form, JSON editor, save/reset
      merchandising.spec.ts    Pin/hide, save as rule, reset
      api-keys.spec.ts         Key CRUD, permissions, scoping
      search-logs.spec.ts      API log viewer, expand, filter, export
      system.spec.ts           Health, indexes, replication, snapshots
      migrate.spec.ts          Migration form UI and validation
      migrate-algolia.spec.ts  Full Algolia migration (needs credentials)
      navigation.spec.ts       Sidebar, routing, dark mode
      cross-page-flows.spec.ts Cross-page data consistency
  e2e-api/                     PURE API — no browser, no page.goto()
    analytics-api-shapes.spec.ts   API response shapes
    analytics-data-api.spec.ts     Data rollup verification (seeded)
    demo-analytics-api.spec.ts     Seed/flush/clear endpoints
  specs/                       BDD specifications (Tier 2)
    *.md                       Feature specs with Given/When/Then
    behaviors/                 Detailed behavior specs
```

**e2e-ui/** — Real Chromium browser, real server, no mocks. Every test interacts with the UI exactly like a human user would. This is our primary test coverage. **If a test uses `page.goto()`, it goes here.**

**e2e-api/** — Pure REST API tests via Playwright's `request` fixture. No browser rendering. No `page.goto()`. HTTP calls only. Tests response shapes, data integrity, and API contracts.

## Prerequisites

- **Flapjack server** running on `localhost:7700`
- **Vite dev server** on `localhost:5177` (started automatically by Playwright unless already running)
- **Algolia credentials** (e2e-api tests only) — set `ALGOLIA_APP_ID` and `ALGOLIA_ADMIN_KEY` in `../../.secret/.env.secret`

## Running Tests

```bash
# E2E-UI tests (real browser, simulated human — primary suite)
npm run test:e2e-ui           # All E2E-UI tests (smoke + full)
npm run test:e2e-ui:smoke     # Just smoke tests (critical paths, ~2 min)
npm run test:e2e-ui:full      # Full E2E-UI suite (10-15 min)

# E2E-API tests (API-level, some require Algolia credentials)
npm run test:e2e-api

# All tests (e2e-ui + e2e-api)
npm test

# Interactive UI mode (recommended for development)
npm run test:ui

# Headed mode (see the browser — use for debugging, not CI)
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

When the server state is unknown (e.g. indexes may or may not exist), use Playwright's `.or()` pattern to wait for one of several valid outcomes:

```typescript
await expect(
  page.getByText('Search Behavior').or(page.getByText(/no indexes/i)),
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

### Adding e2e-api tests

Place API-level tests (no browser rendering) in `tests/e2e-api/`. For tests requiring external services, use the conditional skip pattern:

```typescript
// E2E-API: These tests call REST APIs directly (no browser rendering).
// For real-browser simulated-human tests, see tests/e2e-ui/
import { test, expect } from '../fixtures/auth.fixture';
import { hasAlgoliaCredentials } from '../fixtures/algolia.fixture';

const describeOrSkip = hasAlgoliaCredentials()
  ? test.describe
  : test.describe.skip;

describeOrSkip('My E2E-API Test', () => {
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

### E2E-UI Tests (Real Browser, Simulated Human — PRIMARY SUITE)

- **Smoke** — 7 critical path tests (~2 min): overview loads, search works, nav works, settings/keys/system load, create+delete index
- **Full Suite** — Comprehensive per-page tests (10-15 min):
  - **Overview** — stat cards, index list, create/delete index, templates, export/upload, analytics summary, navigation to index
  - **Search & Browse** — search input, facets filtering, clear filters, breadcrumbs, add documents dialog, analytics toggle
  - **Settings** — searchable attrs, faceting, ranking, JSON editor toggle, compact button, save+persist, reset
  - **API Keys** — CRUD, permissions toggle, copy feedback, index scope, form validation
  - **Analytics** — 6 tabs (overview, searches, no-results, filters, devices, geography), KPI cards, charts, date ranges, filter input, geo drill-down, clear analytics
  - **Navigation** — sidebar links, active states, dark mode, logo, connection dialog
  - **System** — health details, indexes, replication, snapshots
  - **Migrate** — form validation, API key toggle, overwrite switch, error handling
  - **Rules** — list, create via JSON editor, delete via UI, clear all, condition/consequence display
  - **Synonyms** — list, create multi-way + one-way, delete, clear all, search/filter, type badges
  - **Merchandising** — search, pin/hide, save as rule, reset, cross-page verification
  - **Search Logs** — log capture, expand/collapse entries, filter, curl export, clear, view modes
  - **Cross-Page Flows** — settings→search consistency, merchandising→rules, full lifecycle, navigation data integrity

### E2E-API Tests (API-Level, No Browser Rendering)

- **Analytics Pipeline** — API response shapes, click analytics, search→analytics pipeline
- **Analytics Data Verification** — Data rollup integrity, device/geo breakdowns, filter subsets
- **Demo Analytics** — Seed→navigate→verify flow
- **Migrate** — Full Algolia migration E2E (requires Algolia credentials)
