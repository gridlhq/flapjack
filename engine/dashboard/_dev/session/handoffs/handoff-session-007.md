# Session 007 — Test Infrastructure Cleanup & Optimization

**Date:** 2026-02-20
**Branch:** mac2

## What was done

### 1. Fixed Docker E2E false positive (CRITICAL)
`test_docker_e2e.rs` had a `if !docker_available() { return; }` that silently passed when Docker wasn't running, counting as a "green" test in every nextest run despite testing nothing.

**Fix:** Added `#[ignore]` attribute with explanation. Now:
- Test is excluded from default nextest runs (no false positive)
- Fails loudly with `assert!` if Docker isn't available when explicitly run
- Run explicitly via: `cargo nextest run --test test_docker_e2e --run-ignored all`

### 2. Removed SharedFixture dead code
Removed `OnceCell` + `SharedServer` boilerplate from 3 files:
- `test_geo.rs` — removed struct, static, wrapper fn (~15 lines)
- `test_sdk_compat.rs` — same pattern (~15 lines)
- `test_quickstart_api.rs` — same pattern (~15 lines)

Each test now calls `spawn_server().await` directly. The OnceCell pattern never provided any sharing under nextest (process-per-test isolation) — it was just misleading indirection.

### 3. Removed redundant geo test
`test_geo_minimum_around_radius_ignored_with_explicit_radius` was an exact duplicate of the second half of `test_geo_minimum_around_radius_is_floor_not_replacement` (same params: `aroundRadius=500000, minimumAroundRadius=5000000`, same 3 assertions).

### 4. Moved 43 facet tests to lib tests (~65s speed improvement)
The biggest optimization. Moved all pure-flapjack facet tests from `engine/tests/test_facets.rs` (nextest, ~1.5s per test process startup) to `engine/src/integ_tests/test_facets.rs` (in-process lib test, negligible overhead).

- **Before:** 60 facet tests in nextest = ~90s
- **After:** 43 tests in lib (~1s), 17 flapjack_http tests remain in nextest (~25s)
- **Net savings:** ~65s per integration test run

The 17 tests that stayed in integration are those referencing `flapjack_http::dto` or `flapjack_http::handlers` (DTO serialization, params parsing).

### 5. Updated test counts in docs and scripts
- `TESTING.md`: All counts updated (1051 lib, 332 integration, 12 server = 1395 total)
- `./s/test` script: Count comments and --list output updated
- Removed all SharedFixture documentation (no longer applicable)

## Test counts (verified)

| Layer | Before | After |
|-------|--------|-------|
| Lib (`cargo test --lib`) | 988 | **1051** (+63) |
| Integration (`cargo nextest run`) | 376 | **332** (-44) |
| Server binary | 12 | 12 |
| **Total** | **1376** | **1395** |

Note: Total went up by 19 because the old docs had stale counts (claimed 988 lib + 366 integ, but actual was 988 + 376).

## Files changed

### Modified
- `engine/tests/test_docker_e2e.rs` — #[ignore] + assert instead of early return
- `engine/tests/test_geo.rs` — removed SharedFixture + removed duplicate test
- `engine/tests/test_sdk_compat.rs` — removed SharedFixture
- `engine/tests/test_quickstart_api.rs` — removed SharedFixture
- `engine/tests/test_facets.rs` — slimmed to 17 flapjack_http-only tests
- `engine/src/integ_tests/mod.rs` — added `mod test_facets;`
- `engine/docs2/1_STRATEGY/TESTING.md` — updated counts, removed SharedFixture docs
- `engine/_dev/s/test` — updated count comments

### Created
- `engine/src/integ_tests/test_facets.rs` — 43 facet tests moved from integration to lib

## Remaining known issues

1. **Docker E2E test untested in CI** — now correctly #[ignore]'d but should be added to a CI step that has Docker available
2. **5 cross-crate test files still in nextest** — tests importing flapjack_http types can't move to lib without extracting types into flapjack crate
3. **flapjack-replication has a flaky test** — `config::tests::test_load_or_default_no_file` occasionally fails (pre-existing, not caused by this session)
