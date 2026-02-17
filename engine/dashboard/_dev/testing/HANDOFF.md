# Testing Overhaul — Handoff

**Date:** 2026-02-14
**Status:** Infrastructure + tests written, build passes, awaiting real-server verification

---

## What Changed

Replaced all mocked e2e-ui tests with unmocked, real-browser, real-server tests.

### Deleted
- 8 old mocked test files from `tests/e2e-ui/full/` (analytics-clear, api-logs, migrate, multi-index, navigation, search-browse, search-facets-regression, snapshots)
- Old mocking `helpers.ts` (19KB of route-mocking infrastructure)
- Legacy `tests/pages/` directory (16 files)
- `AI_TESTING_METHODOLOGY.md` (wrong project)

### Created / Rewritten
- **Infrastructure:** `seed.setup.ts`, `cleanup.setup.ts`, `helpers.ts` (constants only, no mocks)
- **Config:** `playwright.config.ts` (project dependencies: seed → e2e-ui → cleanup)
- **BDD Specs:** 10 files in `tests/specs/` (56 test scenarios)
- **Smoke Tests:** `tests/e2e-ui/smoke/critical-paths.spec.ts` (7 critical paths)
- **Full Tests:** 10 files in `tests/e2e-ui/full/` (53 tests across all pages)
- **Docs:** `TESTING.md` (lean, 90 lines), `OVERHAUL_CHECKLIST.md`

### Fixed
- `tsconfig.json` — excluded `*.test.ts(x)` files from production build (pre-existing issue)

---

## Test Architecture

```
Playwright project lifecycle:
  seed.setup.ts  →  e2e-ui tests  →  cleanup.setup.ts
  (create data)     (run tests)       (delete data)
```

- **Seed** checks backend health, creates `e2e-products` index, batch-adds 12 products, configures settings/synonyms/rules, seeds 7 days of analytics
- **Tests** run in real Chrome against real backend — zero mocking
- **Cleanup** deletes `e2e-products`, `e2e-temp`, and clears analytics

---

## Test Counts

| Suite | Files | Tests |
|---|---|---|
| Smoke | 1 | 7 |
| Full | 10 | 53 |
| Setup/Teardown | 2 | 3 |
| **Total** | **13** | **63** |

---

## How to Run

**Prerequisites:** Flapjack server on port 7700, Vite dev server on port 5177

```bash
npm run test:smoke        # smoke only (~2 min)
npm run test:e2e-ui       # all e2e-ui (smoke + full)
npm run test:e2e-ui:full  # full suite only
npm run test:ui           # Playwright UI mode (debugging)
```

---

## Remaining Work

1. **Run smoke tests against real server** — verify seed + tests + cleanup lifecycle works end-to-end
2. **Run full suite against real server** — expect some assertions may need tuning for real data
3. **Fine-tune selectors** — some tests use flexible selectors (`getByRole`, `getByText`); real UI may need adjustments
4. **Analytics seeding** — if the `POST /2/analytics/seed` endpoint doesn't exist yet, analytics tests will need the endpoint or alternative seeding

---

## File Locations

- Checklist: `_dev/testing/OVERHAUL_CHECKLIST.md`
- Testing guide: `_dev/testing/TESTING.md`
- BDD specs: `tests/specs/*.md`
- Smoke tests: `tests/e2e-ui/smoke/critical-paths.spec.ts`
- Full tests: `tests/e2e-ui/full/*.spec.ts`
- Test data: `tests/fixtures/test-data.ts`
- Seed/cleanup: `tests/e2e-ui/seed.setup.ts`, `tests/e2e-ui/cleanup.setup.ts`
