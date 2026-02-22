# Session 008 — Test Audit & Fixes

**Date:** 2026-02-20
**Branch:** mac2
**Previous session:** handoff-session-007.md

## Context

Audited all changes from session 007 (test infrastructure cleanup). Found and fixed real bugs + removed redundant tests. Goal: ensure every test is correct, non-redundant, and passing — no manual QA needed.

## What was fixed

### 1. Fixed flaky replication config tests (BUG)

**File:** `flapjack-replication/src/config.rs`

Two tests (`test_load_or_default_no_file` and `test_load_or_default_invalid_json`) were failing intermittently because they read the `FLAPJACK_PEERS` environment variable without holding the `ENV_MUTEX`. When `test_load_or_default_flapjack_peers_env_var` ran concurrently and set `FLAPJACK_PEERS`, these tests saw 1 peer instead of 0.

**Fix:**
- Added `ENV_MUTEX` lock to both tests
- Added explicit `remove_var("FLAPJACK_PEERS")` and `remove_var("FLAPJACK_NODE_ID")` cleanup
- Tests now always pass (verified 100% on multiple runs)

The session 007 handoff incorrectly labeled this as "pre-existing, not caused by this session" — it was actually a race condition that has always existed. Now fixed for good.

### 2. Removed 5 redundant parse_facet_params integration tests

**File:** `engine/tests/test_facets.rs`

Five `test_parse_facet_params_*` tests were exact duplicates of unit tests in `flapjack-http/src/handlers/facets.rs`. Since `parse_facet_params` is a pure function, testing it at the integration level (nextest process-per-test) provided zero additional coverage.

**Action:**
- Removed 3 duplicate tests: `_basic`, `_with_filters`, `_defaults`
- Moved 2 unique tests (`_empty_query`, `_empty_string`) to the unit test module in `handlers/facets.rs`
- Net: 5 fewer integration tests, 2 more unit tests, zero coverage loss

### 3. Updated all test counts in docs and scripts

Updated `TESTING.md` and `./s/test` to reflect accurate counts after changes.

## Audit results — session 007 changes verified

| Change | Status |
|--------|--------|
| Docker E2E `#[ignore]` + `assert!` | Correct |
| SharedFixture removal from test_geo.rs | Clean, no dead code remaining |
| SharedFixture removal from test_sdk_compat.rs | Clean |
| SharedFixture removal from test_quickstart_api.rs | Clean |
| Redundant geo test removal | Correct — was exact duplicate |
| Facet test migration (43 tests to lib) | Correct — all 43 pass in-process |
| Facet integration tests (12 remaining) | All reference flapjack_http types, correctly kept |
| Test count claims in handoff-007 | Verified accurate (43 moved, 17→12 remain) |

## Test counts (verified)

| Layer | Session 007 | Session 008 | Delta |
|-------|-------------|-------------|-------|
| Lib (`cargo test --lib`) | 1051 | **1053** | +2 (moved parse_facet_params) |
| Integration (`cargo nextest run`) | 332 | **327** | -5 (removed redundant) |
| Server binary | 12 | 12 | 0 |
| **Total** | **1395** | **1392** | -3 (net dedup) |

Breakdown: 807 flapjack + 215 flapjack-http + 31 flapjack-replication = 1053 lib

**All 1392 tests pass. Zero failures. Zero flaky tests.**

## Files changed

### Modified
- `flapjack-replication/src/config.rs` — fixed env var race condition in 2 tests
- `engine/tests/test_facets.rs` — removed 5 redundant parse_facet_params tests (17 → 12)
- `flapjack-http/src/handlers/facets.rs` — added 2 parse_facet_params edge case tests
- `engine/docs2/1_STRATEGY/TESTING.md` — updated counts, added flaky test fix to history
- `engine/_dev/s/test` — updated count comments

## Remaining known issues

(Inherited from session 007, unchanged)
1. Docker E2E test untested in CI — correctly `#[ignore]`'d, needs CI step with Docker
2. 5 cross-crate test files still in nextest — need type extraction to move to lib
3. Geo tests stuck in HTTP tier — geo logic lives in flapjack-http handlers
