# Session 006 — Review & Fix Previous Session's Test Infrastructure Work

**Date:** 2026-02-20
**Branch:** mac2
**Focus:** Audited session 005 deliverables, found and fixed critical bugs

---

## Bugs Found & Fixed

### BUG 1 (Critical): SharedFixture crashes under concurrent load
- `cargo test --test test_geo` runs 34 tests as threads in one process
- All 34 threads hit one in-process Axum server simultaneously
- Server can't handle the load → "Connection reset by peer" / "Connection refused"
- **Result:** 19/34 tests fail with SharedFixture, 34/34 pass with nextest
- **Fix:** Removed SharedFixture routing from test runner; all tests now use nextest

### BUG 2 (Critical): SharedFixture applied to files with non-unique index names
- `test_sdk_compat.rs` — 68 tests all use `"products"` index
- `test_quickstart_api.rs` — tests reuse `"movies"`, `"items"`, `"products"`, `"books"`
- With SharedFixture (threads sharing one server), concurrent tests race on same data
- Session 005 handoff incorrectly claimed "All tests use unique index names"
- **Fix:** Added warning comments to SharedFixture code; all 3 files run via nextest

### BUG 3: `--all` flag runs SDK tests twice
- `--all` sets both `RUN_E2E=true` and `RUN_SDK=true`
- Both sections run `test.js` + `contract_tests.js` → duplicate test execution
- **Fix:** SDK section now skips if E2E already ran these tests

### BUG 4: Dead code in test runner
- `EXCLUDE_ARGS` variable computed but never used (replaced by `-E "$FILTER"`)
- **Fix:** Removed dead code along with SharedFixture routing

### BUG 5: Doc inconsistencies
- Strategy table claimed 122 SharedFixture tests / ~7s → was 0 working
- Quick reference described "SharedFixture optimization" that didn't function
- **Fix:** Updated all docs to reflect nextest-only reality

---

## Files Changed

### Modified files
- `engine/_dev/s/test` — Simplified `--integ` to plain nextest; fixed --all dedup; removed dead code
- `engine/tests/test_geo.rs` — Updated SharedFixture comment (warns: don't use cargo test --test)
- `engine/tests/test_sdk_compat.rs` — Updated SharedFixture comment (warns: non-unique index names)
- `engine/tests/test_quickstart_api.rs` — Updated SharedFixture comment (warns: non-unique index names)
- `engine/docs2/1_STRATEGY/TESTING.md` — Corrected SharedFixture docs, Strategy table, Quick Reference
- `TODO_test_infra.md` — Updated checklist with session 006 fixes

### New files
- `engine/dashboard/_dev/session/handoffs/handoff-session-006.md` — this file

---

## Test Results

All tests verified passing after fixes:

| Suite | Count | Result |
|-------|-------|--------|
| Rust unit (lib) | 988+ | PASS |
| Rust integration (nextest) | 376 | PASS (376 passed, 0 skipped) |
| Server binary | 12 | PASS |

---

## What Remains from Session 005

These items were correctly implemented and work:
- Unified test runner (`./s/test`) with 12 flags — verified working
- CLI smoke test (`cli_smoke.sh`) — 17 curl-based checks, correctly wired
- SDK test npm scripts in `sdk_test/package.json` — verified
- Debug script archival to `sdk_test/_archive/` — verified

These are still nice-to-haves:
- Make index names unique in test_sdk_compat.rs / test_quickstart_api.rs
- Investigate Axum in-process server connection limits
- Move pure unit tests from test_facets.rs to lib tests
- Add contract_tests.js to CI pipeline
- Consider removing old `_dev/s/test.sh` and `_dev/s/smoke.sh`

---

## Key Lesson

SharedFixture (`tokio::sync::OnceCell` + `cargo test --test`) requires TWO conditions:
1. **Unique index names** — every test must use its own index (test_sdk_compat fails this)
2. **Server capacity** — the in-process server must handle N concurrent connections (test_geo fails this)

Neither condition was verified before the session 005 changes were declared complete.
The OnceCell code is harmless when run via nextest (process-per-test isolation),
so it was left in place with warning comments.

---

## Checklist

See `TODO_test_infra.md` in repo root for live checklist.
