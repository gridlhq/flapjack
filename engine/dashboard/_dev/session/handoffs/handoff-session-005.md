# Session 005 — Test Infrastructure Overhaul (Implementation)

**Date:** 2026-02-20
**Branch:** mac2
**Focus:** Implemented unified test runner, SharedFixture, CLI smoke test, TESTING.md update

---

## What Was Done

### 1. Unified test runner: `engine/_dev/s/test`
- Single script replaces `run-all-tests.sh`, `test.sh`, `smoke.sh`
- 12 flags: `--unit`, `--integ`, `--server`, `--smoke`, `--sdk`, `--sdk-algolia`, `--dashboard`, `--dashboard-full`, `--e2e`, `--go`, `--all`, `--ci`, `--list`
- Auto-starts/stops flapjack server for tests that need it
- `--integ` routes SharedFixture files to `cargo test --test` and rest to nextest

### 2. SharedFixture pattern: 3 test files converted
- **test_geo.rs** (34 tests): All share one server via `tokio::sync::OnceCell`
- **test_sdk_compat.rs** (69 tests): All share one server (except 1 auth test)
- **test_quickstart_api.rs** (19 tests): All share one server (except 2 auth tests)
- **112 total `spawn_server()` calls replaced** with `shared_server()`
- All compile clean. Backward compatible with nextest (each process inits own).
- **Expected speedup:** ~207s → ~12s when run via `cargo test --test` (threads, not nextest)

### 3. CLI smoke test: `engine/_dev/s/manual-tests/cli_smoke.sh`
- 17 curl-based checks against real binary
- Tests: health, batch add, text search, empty search, get object, 404, settings set/get, filter search, numeric filter, facets, multi-index search, update, delete, list indices, clear, delete index
- Can run standalone (builds + starts server) or against running server (`FJ_ALREADY_RUNNING=true`)
- Wired into `./s/test --e2e`

### 4. TESTING.md updated
- Added Quick Reference table at top with all test types, counts, times, commands
- Added unified test runner command reference
- Fixed `--sdk` documentation (was misleading — only runs Go/PHP, not JS)
- Updated SDK section: classified files as automated, Algolia-required, or debug utilities
- Updated SharedFixture docs with real implementation pattern and timing estimates
- Updated Strategy Overview table with new test types
- Added cli_smoke.sh to manual tests table

### 5. Cleanup
- Archived to `sdk_test/_archive/`: `debug_search.js`, `debug_highlight.mjs`, `test_v4_simple.js`
- Added `test:contract`, `test:all`, `test:validation` scripts to `sdk_test/package.json`

---

## Files Changed

### New files
- `engine/_dev/s/test` — unified test runner (executable)
- `engine/_dev/s/manual-tests/cli_smoke.sh` — CLI smoke test (executable)
- `engine/sdk_test/_archive/debug_search.js` — moved from sdk_test/
- `engine/sdk_test/_archive/debug_highlight.mjs` — moved from sdk_test/
- `engine/sdk_test/_archive/test_v4_simple.js` — moved from sdk_test/

### Modified files
- `engine/tests/test_geo.rs` — SharedFixture (all 34 tests share one server)
- `engine/tests/test_sdk_compat.rs` — SharedFixture (68 tests share one server)
- `engine/tests/test_quickstart_api.rs` — SharedFixture (15 tests share one server)
- `engine/docs2/1_STRATEGY/TESTING.md` — Quick reference, SDK docs, SharedFixture docs
- `engine/sdk_test/package.json` — Added test:contract, test:all, test:validation scripts
- `TODO_test_infra.md` — Updated checklist with completion status

---

## What's Left (nice-to-haves)

1. **Timing verification** — Run `./s/test --integ` and compare against raw `cargo nextest run` to validate the SharedFixture speedup claim
2. **Move pure unit tests from test_facets.rs** — 20 tests that just test DTO serialization/deserialization with no IO could be lib tests
3. **CI integration** — Add `contract_tests.js` to CI pipeline
4. **Consider removing** old `_dev/s/test.sh` and `_dev/s/smoke.sh` (replaced by `./s/test`)

---

## Key Decision: Why `cargo test --test` instead of nextest for SharedFixture

Nextest runs each test in a separate OS process. `tokio::sync::OnceCell` is process-local, so with nextest each test still creates its own server — no benefit from sharing.

`cargo test --test test_geo` runs all tests in that file as threads in one process. The OnceCell is initialized once, and all 34 tests share the same server instance. The savings:
- Binary load: paid once instead of 34 times (~1.5s each = ~50s saved)
- Server init: paid once instead of 34 times (~0.2s each = ~7s saved)
- Total: ~58s → ~2s for test_geo alone

The `./s/test --integ` flag handles this routing automatically.

---

## Checklist

See `TODO_test_infra.md` in repo root for live checklist.
