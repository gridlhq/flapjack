# Dashboard Testing

**Requires:** Flapjack server on port 7700 + Vite dev server on port 5177

---

## CRITICAL: Two Test Categories

| Category | Directory | What it does | Opens a real browser? |
|----------|-----------|--------------|----------------------|
| **E2E-UI** | `tests/e2e-ui/` | Simulated-human interaction in a **real rendered Chromium browser**. Clicks, types, navigates exactly like a user would. **NO mocks, NO code-based browser manipulation.** | **YES** |
| **E2E-API** | `tests/e2e-api/` | Pure REST API calls via Playwright `request` fixture. Verifies response shapes, data integrity, API contracts. **Never opens a browser.** | **NO** |

**When we say "e2e-ui tests" we mean: non-mocked, simulated-human, real-browser tests.**
**When we say "e2e-api tests" we mean: HTTP-only API tests. No `page.goto()`. No browser rendering.**

If a test uses `page.goto()`, `page.click()`, `page.fill()`, or any browser locator — it belongs in `e2e-ui/`, period.

---

## Three Test Tiers (3-Tier BDD)

### Tier 1: BDD Specs (`tests/specs/*.md`)
Human-readable step lists. Each test is a numbered list of actions + expected results.
Written BEFORE the test code. Serves as the source of truth.

### Tier 2: Test Implementation
- **E2E-UI** (`tests/e2e-ui/**/*.spec.ts`) — real browser Playwright tests derived from BDD specs.
- **E2E-API** (`tests/e2e-api/**/*.spec.ts`) — pure API tests for data integrity and contract verification.

### Tier 3: Unit Tests (`src/**/*.test.{ts,tsx}`)
Fast, isolated, mock external deps. Run on every save.

---

## Directory Structure

```
tests/
  e2e-ui/                    REAL BROWSER — simulated human clicks
    seed.setup.ts            Seeds test data before e2e-ui tests
    cleanup.setup.ts         Cleans up after e2e-ui tests
    helpers.ts               API constants + re-exports (NO mocks)
    smoke/                   7 critical-path tests (~2 min)
    full/                    Comprehensive per-page tests (~10-15 min)
      overview.spec.ts       14 tests — index list, stats, create/delete
      search.spec.ts         22 tests — search, facets, pagination
      analytics.spec.ts      14 tests — 6 tabs, KPIs, charts, date ranges
      analytics-deep.spec.ts 17 tests — deep data verification (seeded data values)
      rules.spec.ts          12 tests — CRUD, JSON editor, conditions
      synonyms.spec.ts       10 tests — CRUD, types, search/filter
      settings.spec.ts       10 tests — attrs, faceting, ranking, JSON, save
      merchandising.spec.ts  10 tests — pin/hide, save as rule, reset
      api-keys.spec.ts       10 tests — CRUD, permissions, copy, scoping
      search-logs.spec.ts    9 tests  — log viewer, expand, filter, export
      system.spec.ts         16 tests — health, indexes, replication, snapshots
      migrate.spec.ts        13 tests — form validation, toggle, errors
      migrate-algolia.spec.ts 2 tests — full Algolia migration flow (needs creds)
      navigation.spec.ts     14 tests — sidebar, routing, dark mode
      cross-page-flows.spec.ts 8 tests — cross-page data consistency
  e2e-api/                   PURE API — no browser, no page.goto()
    analytics-api-shapes.spec.ts   API response shape verification
    analytics-data-api.spec.ts     Seeded data rollup verification
    demo-analytics-api.spec.ts     Seed/flush/clear endpoint tests
  fixtures/
    auth.fixture.ts          Pre-seeds localStorage auth
    test-data.ts             Products, synonyms, rules for seeding
    analytics-seed.ts        Analytics data seeding helpers
    algolia.fixture.ts       Algolia migration helpers (needs creds)
  specs/                     BDD specifications (human-readable test plans)
    behaviors/               Detailed behavior specs
```

---

## Running Tests

| Command | What | Browser? | Speed |
|---|---|---|---|
| `npm run test:unit` | Unit tests (watch) | No | < 1s |
| `npm run test:e2e-ui:smoke` | Critical paths | **Yes** | ~2 min |
| `npm run test:e2e-ui:full` | All pages | **Yes** | ~10-15 min |
| `npm run test:e2e-ui` | Smoke + full | **Yes** | ~12-17 min |
| `npm run test:e2e-api` | API contract tests | No | ~5-8 min |
| `npm test` | All e2e-ui (default) | **Yes** | ~12-17 min |

## Debugging

```bash
npm run test:ui            # Playwright UI mode (real-time, time-travel)
npm run test:headed        # See the browser (watch it click)
npm run test:debug         # Step-through with Playwright Inspector
```

---

## Key Rules

1. **No mocking in e2e-ui tests.** Real browser, real server, real data.
2. **No browser in e2e-api tests.** Pure HTTP. No `page.goto()`, no `page.click()`.
3. **Seed data in setup, assert against it.** Test data comes from `test-data.ts`.
4. **Each test is independent.** No shared state between tests.
5. **Tests that create data must clean up.** Use `test.afterAll` for temp indexes.
6. **BDD specs come first.** Write the spec, then the test.
7. **No sleeps.** Never use `page.waitForTimeout()`. Use Playwright auto-retry.
