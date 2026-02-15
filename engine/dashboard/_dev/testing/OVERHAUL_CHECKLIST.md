# Testing Overhaul Checklist

**Goal:** Replace mocked e2e-ui tests with real-browser, real-server, unmocked tests.

---

## Phase 1: Clean Up
- [x] Delete old mocked e2e-ui tests (tests/e2e-ui/smoke/, tests/e2e-ui/full/)
- [x] Delete old mocking helpers.ts
- [x] Delete legacy page tests (tests/pages/)
- [x] Delete old BDD specs
- [x] Delete AI_TESTING_METHODOLOGY.md (wrong project)

## Phase 2: Infrastructure
- [x] Create helpers.ts (API constants, no mocks)
- [x] Create seed.setup.ts (seeds test data into real backend)
- [x] Create cleanup.setup.ts (teardown)
- [x] Update playwright.config.ts (project dependencies: seed → e2e-ui → cleanup)
- [x] Update package.json test scripts
- [x] Keep auth.fixture.ts (works with real server)
- [x] Keep test-data.ts (seed data source)

## Phase 3: BDD Specs
- [x] overview.md
- [x] search.md
- [x] settings.md
- [x] analytics.md
- [x] synonyms.md
- [x] rules.md
- [x] merchandising.md
- [x] api-keys.md
- [x] search-logs.md
- [x] system.md

## Phase 4: Test Implementation
- [x] Smoke: critical-paths.spec.ts (7 critical paths)
- [x] Full: overview.spec.ts
- [x] Full: search.spec.ts
- [x] Full: settings.spec.ts
- [x] Full: analytics.spec.ts
- [x] Full: synonyms.spec.ts
- [x] Full: rules.spec.ts
- [x] Full: merchandising.spec.ts
- [x] Full: api-keys.spec.ts
- [x] Full: search-logs.spec.ts
- [x] Full: system.spec.ts

## Phase 5: Documentation
- [x] Rewrite TESTING.md (lean, 3-tier)
- [x] Create handoff doc
- [x] Update MEMORY.md

## Phase 6: Verification
- [x] Build passes (`npm run build`)
- [x] Playwright discovers all 63 tests (7 smoke + 53 full + seed + cleanup + teardown)
- [ ] Smoke tests pass against real server
- [ ] Full e2e-ui tests pass against real server
