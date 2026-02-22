# Session 009 — Audit & Fix Documentation Lies

**Date:** 2026-02-20
**Previous:** [handoff-session-008.md](handoff-session-008.md)

---

## Mission

Audit all previous session work (005-008) for correctness. Verify every documented test count matches reality. Fix all lies in checklists.

---

## Bugs Found & Fixed

### 1. E2E_UI_COVERAGE_CHECKLIST.md — Multiple False Claims

The checklist was a minefield of incorrect numbers. Every count was verified by `grep -c 'test(' + 'test.skip('` against the actual spec files.

**Issues fixed:**

| Issue | Before (Wrong) | After (Correct) |
|-------|---------------|-----------------|
| Header total | 236 tests (7 smoke + 229 full) | 234 tests (7 smoke + 227 full) — 227 active + 7 skipped |
| api-keys.spec.ts | 10 tests, all "Done" | 3 active + 7 `test.skip()` (backend `/1/keys` not implemented) |
| search.spec.ts | 22 tests | 20 tests (2 removed: no Search button, no filter toggle) |
| merchandising.spec.ts per-page | 10 tests listed | 14 tests listed (4 drag-and-drop tests were added but never documented) |
| query-suggestions.spec.ts | "1-9 \| Done" (no breakdown) | All 9 tests listed individually |
| Summary table total | 236 | 227 active + 7 skipped = 234 |
| API Keys coverage | "Full" | "Partial (needs `/1/keys` backend)" |

**Root cause:** Previous sessions added/removed tests from spec files without updating the checklist. The api-keys issue was the worst — 7 tests marked "Done" were actually `test.skip()` because the server doesn't support the `/1/keys` API yet.

### 2. TESTING.md — Wrong Dashboard Counts

| Field | Before (Wrong) | After (Correct) |
|-------|---------------|-----------------|
| Dashboard E2E smoke | ~30 | 7 |
| Dashboard E2E full | ~221 | 227 (220 active + 7 skipped) |
| Strategy overview smoke row | ~30 | 7 |
| Debug utilities table | Listed deleted files | Updated to note they're archived |

**Root cause:** The "~30" smoke count was never accurate — there are only 7 tests in `critical-paths.spec.ts`. The "~221" full count was from before drag-and-drop and query-suggestions tests were added.

---

## Verification — All Tests Pass

| Layer | Count | Status |
|-------|:-----:|--------|
| Lib (`cargo test --lib`) | 1053 (807 + 215 + 31) | All pass |
| Integration (`cargo nextest run`) | 327 (326 + 1 docker ignored) | All pass |
| Server binary (`cargo test -p flapjack-server`) | 12 | All pass |
| **Rust total** | **1392** | **Zero failures, zero flaky** |

Dashboard E2E (not run in this session — requires server + browser):
- Smoke: 7 tests in 1 file
- Full: 220 active + 7 skipped in 18 files
- Total: 234 tests in 19 files

### Redundancy Check

Searched all test files across unit (`#[cfg(test)]`), lib integration (`src/integ_tests/`), and nextest integration (`tests/`) layers. **No redundant tests found.** The existing split is intentional and well-documented:
- Unit tests: pure helper functions
- Lib integration: IndexManager-level, in-process
- Nextest integration: HTTP/DTO, process-per-test

---

## Session 005-008 Audit Verdict

| Session | Change | Verdict |
|---------|--------|---------|
| 005 | SharedFixture pattern | Correctly reverted in 006 |
| 005 | Unified test runner (`./s/test`) | Correct, counts verified |
| 006 | SharedFixture removal | Correct |
| 007 | Docker E2E `#[ignore]` fix | Correct |
| 007 | SharedFixture dead code removal | Correct |
| 007 | Redundant geo test removal | Correct |
| 007 | 43 facet tests moved to lib | Correct |
| 008 | ENV_MUTEX fix in config.rs | Correct and verified |
| 008 | 5 redundant parse_facet_params removed | Correct |
| **All** | **Documented test counts** | **WRONG — fixed in this session** |

---

## Key Files Modified

| File | Change |
|------|--------|
| `dashboard/tests/E2E_UI_COVERAGE_CHECKLIST.md` | Fixed all test counts, marked skipped tests, added missing entries |
| `engine/docs2/1_STRATEGY/TESTING.md` | Fixed dashboard E2E counts (smoke: ~30→7, full: ~221→227) |

## Key File Paths

| Doc | Path |
|-----|------|
| This handoff | `engine/dashboard/_dev/session/handoffs/handoff-session-009.md` |
| Previous handoff | `engine/dashboard/_dev/session/handoffs/handoff-session-008.md` |
| E2E coverage checklist | `engine/dashboard/tests/E2E_UI_COVERAGE_CHECKLIST.md` |
| Test strategy | `engine/docs2/1_STRATEGY/TESTING.md` |
| Browser test standards | `engine/dashboard/BROWSER_TESTING_STANDARDS_2.md` |
| Test runner script | `engine/_dev/s/test` |

---

## Known Gaps Remaining

1. **API Keys backend** — 7 E2E tests are `test.skip()` waiting for `/1/keys` CRUD implementation
2. **Docker E2E** — only runs when Docker is available + image is built; skipped in normal test runs
3. **Geo tests** — stuck in HTTP tier (geo logic in `flapjack-http`, can't move to lib without refactor)
4. **No real two-node E2E test** — replication tests use mocks, not actual multi-node deployment
