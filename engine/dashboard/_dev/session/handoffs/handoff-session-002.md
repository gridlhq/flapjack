# Session Handoff — Analytics E2E Test Hardening

**Date:** 2026-02-20
**Focus:** Fix 6 real problems found in critical review of Phase 4 Analytics E2E tests

---

## Context

A critical review of the Analytics E2E tests identified 6 issues ranging from false-positive risks to missing enforcement. This session fixed all of them.

---

## What Was Done

### 1. CRITICAL: Created ESLint Enforcement Config

**Problem:** `BROWSER_TESTING_STANDARDS_2.md` claimed Layer 1 ESLint enforcement via a config at `ui/browser-tests-unmocked/eslint.config.mjs`. That file did not exist. Zero ESLint config anywhere in the test directories. Nothing prevented `page.evaluate()` or `{ force: true }` from being added to spec files.

**Solution:**
- Installed `eslint`, `eslint-plugin-playwright`, `typescript-eslint` as devDependencies
- Created `tests/e2e-ui/eslint.config.mjs` with TypeScript parser support
- Rules enforce: no-eval, no-element-handle, no-force-option, no-page-pause
- Custom `no-restricted-syntax` rules ban: CSS class selectors (`.className`), XPath (`//`), attribute selectors (`[attr=val]`), API calls in specs (`request.*`), `waitForTimeout`, `dispatchEvent`, `setExtraHTTPHeaders`
- Tag-based locators (`table tbody tr`, `svg`, `th`, `td`) are correctly allowed for row-scoping
- Added `npm run lint:e2e` script to package.json
- Updated `BROWSER_TESTING_STANDARDS_2.md` to reference correct path

**Verification:** `npm run lint:e2e` reports 0 errors on analytics spec files after fixes.

### 2. CRITICAL: Removed Silent Test Skipping (False Positives)

**Problem:** Multiple tests used `if (await element.isVisible().catch(() => false))` guards, meaning broken features silently passed:
- `analytics.spec.ts:228-232` — Filters expansion: `if (!hasData) { test.skip() }`
- `analytics.spec.ts:268` — Country filter: `if (await countryFilter.isVisible().catch(...))`
- `analytics.spec.ts:285` — Device filter: same pattern
- `analytics.spec.ts:341` — Clear Analytics button: same pattern
- `analytics-deep.spec.ts:88-92` — Delta badges: conditional

**Solution:** All guards replaced with hard assertions:
- Country filter: `await expect(countryFilter).toBeVisible({ timeout: 10_000 })` — seeded geo data means this MUST be present
- Device filter: same treatment — seeded device data means this MUST be present
- Clear button: uses `page.getByTestId('clear-btn')` with hard assertion
- Filters expansion: hard assert on first row visibility, no `test.skip()`
- Delta badges: hard assert `await expect(deltaBadge).toBeVisible()` — seeded data provides two periods

### 3. HIGH: Replaced CSS Class Selectors with data-testid

**Problem:** Tests used `.locator('.text-2xl')`, `.locator('.text-3xl')`, `.locator('.text-lg')`, `.locator('.h-64')`, `.locator('.recharts-responsive-container')` extensively. If Tailwind classes change, tests break for the wrong reason.

**Solution — Component (Analytics.tsx):**
- Added `data-testid="kpi-value"` to KPI card value `<span>` in `KpiCard` component
- Added `data-testid="rate-value"` to no-result rate banner percentage `<span>`
- Added `data-testid="device-count"` to device platform count `<div>`
- Added `data-testid="device-pct"` to device platform percentage `<div>`
- Added `data-testid="device-chart"` to device chart container `<div>`
- Added `data-testid="flush-btn"` to flush/update button
- Added `data-testid="clear-btn"` to clear analytics button
- Added `data-testid="filters-no-results"` to the filters-causing-no-results `<Card>`

**Solution — Tests:**
- All `.locator('.text-2xl')` → `.getByTestId('kpi-value')`
- All `.locator('.text-3xl')` → `.getByTestId('rate-value')`
- All `.locator('.text-lg')` → `.getByTestId('device-pct')`
- All `.locator('.h-64')` → `.getByTestId('device-chart')`
- Chart assertions use `.locator('svg')` (tag, not class)
- Flush/Clear buttons use `.getByTestId('flush-btn')` / `.getByTestId('clear-btn')`

### 4. HIGH: Fixed Chart Test Accepting Empty State

**Problem:** `analytics.spec.ts:254-258` used `svg.or(noData).toBeVisible()` — a broken chart showing "No data available" still passed.

**Solution:** Hard assertion on `svg` only. With seeded data, the chart MUST render SVG. Applied to both search volume chart and NRR chart tests.

### 5. HIGH: Added "Filters Causing No Results" Test Coverage

**Problem:** `Analytics.tsx:820-849` renders a "Filters Causing No Results" section. No test touched it.

**Solution:** Added new test `'Filters tab shows "Filters Causing No Results" section when data exists'` in analytics.spec.ts. Added `data-testid="filters-no-results"` to the component. Test verifies heading, column headers, and data rows when the section renders.

Note: This section is conditionally rendered based on `useFiltersNoResults` hook data. The conditional in the test is acceptable here because the data genuinely may not exist (unlike country/device filters which have explicit seeded data).

### 6. MEDIUM: Added Functional Flush Button Test

**Problem:** `analytics.spec.ts:317-323` only checked the flush button existed. Never clicked it.

**Solution:** New test `'Flush button triggers analytics refresh'`:
1. Asserts button visible via `data-testid="flush-btn"`
2. Clicks the button
3. Verifies loading state ("Updating..." text appears)
4. Verifies completion ("Update" text returns)
5. Verifies KPI data still renders after refresh

---

## Files Changed

### Created
- `tests/e2e-ui/eslint.config.mjs` — ESLint enforcement config for spec files
- `_dev/session/handoffs/handoff-session-002.md` — this file

### Modified
- `src/pages/Analytics.tsx` — 8 `data-testid` attributes added (kpi-value, rate-value, device-count, device-pct, device-chart, flush-btn, clear-btn, filters-no-results)
- `tests/e2e-ui/full/analytics.spec.ts` — Complete rewrite: 19→21 tests, all CSS selectors removed, all conditionals removed, flush test functional, filters-no-results test added
- `tests/e2e-ui/full/analytics-deep.spec.ts` — Complete rewrite: 15 tests, all CSS selectors removed, delta badge hard assertion
- `BROWSER_TESTING_STANDARDS_2.md` — Fixed ESLint config path, updated enforcement section, updated file structure
- `tests/E2E_UI_COVERAGE_CHECKLIST.md` — Updated analytics test counts (196→198), added quality standards for CSS selectors and ESLint
- `package.json` — Added `lint:e2e` script, added eslint/eslint-plugin-playwright/typescript-eslint devDependencies

---

## Test Count Changes

| File | Before | After | Change |
|------|--------|-------|--------|
| analytics.spec.ts | 19 | 21 | +2 (flush functional, filters-no-results) |
| analytics-deep.spec.ts | 15 | 15 | 0 (hardened, no new tests) |
| **Total E2E-UI** | **196** | **198** | **+2** |

---

## Issue Resolution Summary

| Issue | Severity | Status | Fix |
|-------|----------|--------|-----|
| ESLint enforcement missing | CRITICAL | **FIXED** | Created `tests/e2e-ui/eslint.config.mjs` + `npm run lint:e2e` |
| Silent test skipping | CRITICAL | **FIXED** | All conditionals → hard assertions |
| CSS class locators | HIGH | **FIXED** | 8 `data-testid` attrs added, all selectors replaced |
| Chart accepts empty state | HIGH | **FIXED** | Removed OR, assert SVG only |
| Filters-no-results untested | HIGH | **FIXED** | New test + `data-testid="filters-no-results"` |
| Flush button untested functionally | MEDIUM | **FIXED** | Click + verify loading state + completion |
| Error states untested | MEDIUM | NOT FIXED | Requires browser-mocked tests (hard to reproduce deterministically) |
| Breadcrumb nav untested | LOW | NOT FIXED | Low risk, reserved slot (#21) in checklist |
| No-indexes empty state untested | LOW | NOT FIXED | Requires special setup (no indexes) |

---

## Remaining Work (Not Addressed)

1. **Error states** (MEDIUM) — Backend 500s, timeouts. Requires `browser-tests-mocked/` setup (mock server responses). Not addressable with real-server tests.
2. **Breadcrumb navigation** (LOW) — Slot reserved as test #21 in analytics.spec.ts
3. **No-indexes empty state** (LOW) — Requires running with zero indexes, conflicts with other tests
4. **ESLint compliance across ALL spec files** — The analytics files now pass. Other spec files (api-keys, search, etc.) may have CSS class violations. Run `npm run lint:e2e` to check and fix incrementally.

---

## Verification Commands

```bash
cd engine/dashboard

# Verify ESLint passes on analytics files
npm run lint:e2e

# Run analytics E2E-UI tests
npx playwright test tests/e2e-ui/full/analytics.spec.ts tests/e2e-ui/full/analytics-deep.spec.ts

# Run full suite
npm test
```
