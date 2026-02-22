# Handoff: E2E Test Timing & Duration Optimization

**Date:** 2026-02-20
**Status:** Analysis complete, ready for optimization work
**Suite:** `engine/dashboard/tests/e2e-ui/full/`
**Config:** `engine/dashboard/playwright.config.ts`

---

## Current Baseline

| Metric | Value |
|--------|-------|
| Wall clock | **49.5s** |
| Sum of all test durations | 107.5s |
| Parallelism efficiency | **2.17x** (ideal = 3.0x with 3 workers) |
| Tests | 221 passed, 7 skipped |
| Workers | 3 (configured in playwright.config.ts line 19) |
| Worker load balance | Excellent — 314ms gap between busiest and least busy |

---

## Per-File Breakdown (sorted by total time)

```
  11617ms  28 tests   415ms avg  analytics.spec.ts
  11599ms  16 tests   725ms avg  overview.spec.ts
  10240ms   4 tests  2560ms avg  connection-health.spec.ts       ** OUTLIER
   8447ms  24 tests   352ms avg  analytics-deep.spec.ts
   8333ms   9 tests   926ms avg  query-suggestions.spec.ts
   6998ms  14 tests   500ms avg  navigation.spec.ts
   6655ms  19 tests   350ms avg  search.spec.ts
   6454ms   3 tests  2151ms avg  api-keys.spec.ts (+7 skipped)  ** OUTLIER (backend 404)
   5419ms  14 tests   387ms avg  merchandising.spec.ts
   4507ms   8 tests   563ms avg  cross-page-flows.spec.ts
   4469ms  11 tests   406ms avg  search-logs.spec.ts
   4458ms  16 tests   279ms avg  system.spec.ts
   3700ms  12 tests   308ms avg  rules.spec.ts
   3425ms  10 tests   342ms avg  synonyms.spec.ts
   3211ms  13 tests   247ms avg  migrate.spec.ts
   3094ms  11 tests   281ms avg  settings.spec.ts
   2828ms   5 tests   566ms avg  auth-flow.spec.ts
   1158ms   2 tests   579ms avg  migrate-algolia.spec.ts
    548ms   1 tests   548ms avg  seed.setup.ts
    341ms   1 tests   341ms avg  cleanup.setup.ts
```

---

## Top 10 Slowest Individual Tests

| Duration | File | Test |
|----------|------|------|
| **7,939ms** | connection-health.spec.ts | recovers from disconnected state when server comes back |
| **6,596ms** | overview.spec.ts | create new index e2e-temp, verify it appears, then delete it |
| **2,895ms** | query-suggestions.spec.ts | rebuild button triggers a build and shows toast |
| 2,250ms | api-keys.spec.ts | toggling permissions updates selection badges |
| 2,201ms | api-keys.spec.ts | create key dialog shows all form sections |
| 2,170ms | navigation.spec.ts | clicking sidebar API Keys navigates to keys page |
| 2,003ms | api-keys.spec.ts | API keys page loads and shows heading and create button |
| 1,999ms | connection-health.spec.ts | shows disconnected banner when server is unreachable |
| 1,446ms | cross-page-flows.spec.ts | create index, add documents, search, then delete |
| 1,224ms | query-suggestions.spec.ts | seeded config renders in the list after navigation |

---

## Duration Distribution

```
  Under 250ms: 54 tests (24%)
  250-500ms:  134 tests (61%)     <-- the bulk
  Over 500ms:  33 tests (15%)     <-- optimization targets
```

---

## Navigation Overhead Analysis

Every test in a `describe` block re-runs `beforeEach` which typically navigates to a page and waits for it to load. This is the single biggest time cost. The "fastest test per file" approximates this overhead because its test body is trivial.

```
File                              Tests  Nav/each  Total Overhead  % of File Time
analytics.spec.ts                  28    ~275ms    7,700ms         66%
analytics-deep.spec.ts             24    ~272ms    6,528ms         77%
search.spec.ts                     19    ~221ms    4,199ms         63%
search-logs.spec.ts                11    ~366ms    4,026ms         90%
system.spec.ts                     16    ~210ms    3,360ms         75%
overview.spec.ts                   16    ~211ms    3,376ms         29%
merchandising.spec.ts              14    ~218ms    3,052ms         56%
migrate.spec.ts                    13    ~210ms    2,730ms         85%
rules.spec.ts                      12    ~203ms    2,436ms         66%
settings.spec.ts                   11    ~205ms    2,255ms         73%
synonyms.spec.ts                   10    ~213ms    2,130ms         62%
navigation.spec.ts                 14    ~137ms    1,918ms         27%
```

**Key insight:** Navigation overhead accounts for ~48,000ms (45%) of the total 107,500ms test time. Files with many short tests (analytics, analytics-deep, search-logs, migrate, system) pay the highest tax.

---

## Optimization Opportunities

### 1. Reduce Navigation Overhead — `test.describe.serial` + Shared State (HIGH IMPACT)

For files where tests are independent reads of the same page (no mutations), group them into a single `test.describe.serial` block that navigates ONCE and reuses the page.

**Best candidates** (many tests, high overhead %, no mutations between tests):

| File | Tests | Nav Overhead | Savings if shared |
|------|-------|-------------|-------------------|
| analytics-deep.spec.ts | 24 | 6,528ms (77%) | ~6,000ms |
| search-logs.spec.ts | 11 | 4,026ms (90%) | ~3,700ms |
| system.spec.ts | 16 | 3,360ms (75%) | ~3,100ms |
| migrate.spec.ts | 13 | 2,730ms (85%) | ~2,500ms |
| settings.spec.ts (read-only tests) | 8 of 11 | ~1,640ms | ~1,400ms |

**Approach:** For read-only test groups, use `test.describe.serial` with a single `test.beforeAll` for navigation. Individual tests then just assert on the already-loaded page. Playwright's `test.describe.serial` ensures tests in the block run in order on the same worker.

Example pattern:
```typescript
test.describe.serial('Analytics Deep — Overview Tab', () => {
  let page: Page;
  test.beforeAll(async ({ browser }) => {
    page = await browser.newPage();
    // auth setup...
    await page.goto(`/index/e2e-products/analytics`);
    await expect(page.getByTestId('kpi-cards')).toBeVisible({ timeout: 10000 });
  });
  test.afterAll(async () => { await page.close(); });

  test('KPI cards show non-zero values', async () => { /* use `page` directly */ });
  test('search volume chart renders SVG', async () => { /* reuse same page */ });
  // etc.
});
```

**Estimated savings: ~15-17 seconds of test time → ~5-6s wall clock reduction.**

### 2. Fix Connection Health Tests (HIGH IMPACT, 2 tests)

`connection-health.spec.ts` has only 4 tests but consumes **10,240ms** (10% of total).

- "recovers from disconnected state" = **7,939ms** — This test likely uses `page.route()` to block health checks, waits for the banner, then unblocks and waits for recovery. The long duration is probably from polling intervals and timeout waits.
- "shows disconnected banner" = **1,999ms** — Same pattern.

**Approach:** Investigate the health polling interval in the dashboard code. If it's e.g. 5 seconds, the test waits for 1-2 poll cycles. Options:
  - Reduce the poll interval during tests (env var or intercepted config)
  - Use `page.route()` to trigger immediate re-check after unblocking
  - Or accept these times as inherent to what's being tested

### 3. Fix API Keys Navigation Overhead (~6s wasted)

`api-keys.spec.ts` has 3 passing tests averaging **2,151ms each**. The `/keys` page calls `GET /1/keys` which returns 404 (unimplemented). The browser likely retries or has error-handling delays.

**Approach:** The beforeEach navigates to `/keys` and waits for the heading. The 2s overhead per test suggests the page takes ~2s to render the empty state after the 404. Consider:
  - Route-intercepting `/1/keys` to return `{ keys: [] }` (200) for faster empty-state rendering
  - Or just accept the overhead since there are only 3 tests

### 4. Overview "Create + Delete Index" Test (6.6s single test)

`overview.spec.ts` line ~create new index: **6,596ms**. This creates an index via the UI, waits for it to appear, then deletes it. The time is split between:
  - Index creation API roundtrip
  - Polling/waiting for the index to appear in the list
  - Deletion API roundtrip + UI refresh

**Approach:** This is a legitimate E2E flow test. Could potentially be moved to a separate serial describe block that shares setup. Limited optimization potential without sacrificing coverage.

### 5. Increase Worker Count (EASY WIN)

Currently 3 workers (playwright.config.ts line 19). The simulated load balance shows nearly perfect distribution. Bumping to **4 or 5 workers** would reduce wall clock proportionally since there's no contention on the server (it handles concurrent requests fine).

**Current:** 49.5s wall, 107.5s total → 2.17x efficiency
**At 4 workers:** ~107.5/4 = ~27s wall (optimistic) + overhead → ~30-33s realistic
**At 5 workers:** ~107.5/5 = ~21.5s wall (optimistic) + overhead → ~25-28s realistic

Risk: more workers = more concurrent browser instances = more RAM. On a dev machine with 36GB+ system memory (per health check), this should be fine.

### 6. Group Tab-Switching Tests to Avoid Redundant Navigation

`analytics.spec.ts` has 28 tests that navigate to the analytics page. Many switch to specific tabs (Searches, Filters, Devices, Geography). Currently each test navigates to the analytics page from scratch, then clicks a tab.

**Approach:** Group by tab. All "Searches tab" tests share one navigation + tab click. All "Filters tab" tests share another. This turns 28 navigations into ~6 (one per tab group + overview).

### 7. Playwright Config Tuning

Consider adding to `playwright.config.ts`:
```typescript
use: {
  // ... existing config
  launchOptions: {
    args: ['--disable-gpu', '--disable-dev-shm-usage'],  // faster in headless
  },
  actionTimeout: 10_000,  // catch stuck actions earlier (default 0 = infinite)
},
```

---

## Priority Ranking

| # | Optimization | Est. Impact | Effort | Risk |
|---|-------------|-------------|--------|------|
| 1 | Increase workers to 4-5 | -10-15s wall | Trivial (1 line) | Low |
| 2 | Serial groups for read-only tests | -5-6s wall | Medium (restructure 5 files) | Low |
| 3 | Group analytics tests by tab | -3-4s wall | Medium (restructure 1 file) | Low |
| 4 | Connection health poll tuning | -5-8s total | Medium | Medium |
| 5 | API keys route intercept for fast 200 | -4s total | Easy | Low |
| 6 | actionTimeout config | Better failure UX | Trivial | None |

**Combined estimated impact: 49.5s → ~25-30s wall clock (40-50% reduction)**

---

## Files to Modify

- `playwright.config.ts` — worker count, actionTimeout
- `analytics-deep.spec.ts` — serial groups (24 tests, biggest overhead)
- `analytics.spec.ts` — tab-grouped serial blocks (28 tests)
- `search-logs.spec.ts` — serial group (11 tests, 90% overhead)
- `system.spec.ts` — serial group (16 tests, 75% overhead)
- `migrate.spec.ts` — serial group (13 tests, 85% overhead)
- `connection-health.spec.ts` — investigate poll interval
- `api-keys.spec.ts` — route intercept for faster empty state

---

## Raw Data

Full JSON test results available at: `/tmp/pw-results.json`
(Regenerate by running: `PLAYWRIGHT_JSON_OUTPUT_NAME=/tmp/pw-results.json npx playwright test --project=e2e-ui tests/e2e-ui/full/ --reporter=json`)

Playwright config: `engine/dashboard/playwright.config.ts` (3 workers, fullyParallel, html reporter)
ESLint config: `engine/dashboard/tests/e2e-ui/eslint.config.mjs`
