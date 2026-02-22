# Session 004 — Test Infrastructure Overhaul

**Date:** 2026-02-20
**Branch:** mac2
**Focus:** Test runner consolidation, timing optimization, documentation

---

## Context

User reviewed test timing data and identified several problems:
- Integration tests take ~1s each due to nextest process-per-test overhead (1.5s binary load + spawn_server per test)
- `run-all-tests.sh --sdk` only runs Go tests, completely ignores JS SDK tests in `sdk_test/`
- No comprehensive test runner with fine-grained flags
- No CLI-level smoke test (start real binary, curl it)
- TESTING.md lacks a quick-reference "how to run each type" section
- Many files in `sdk_test/` are debug utilities, not real tests

## SDK Test File Audit

| File | Type | Algolia creds? | Wire up? |
|------|------|---------------|----------|
| `test.js` | Real test | No | Already `npm test` |
| `contract_tests.js` | Real test | No | **YES — not wired up** |
| `test_algolia_migration.js` | Real test | Yes | Keep manual (creds) |
| `algolia_validation.js` | Real test (15 tests, 4 suites) | Yes (or cache) | Keep manual (creds) |
| `race_test.js` | Quick check (17 lines) | No | Not worth it |
| `test_exhaustive_fields.js` | Debug utility | Yes | No |
| `test_algolia_multi_pin.js` | Debug/exploratory | Yes | No |
| `test_v4_simple.js` | Outdated (CJS, SDK v4) | No | No — outdated |
| `debug_search.js` | Debug utility | No | No |
| `debug_highlight.mjs` | Debug utility (broken import) | Yes | No |

---

## Checklist

See `TODO_test_infra.md` in repo root for live checklist.

---

## Key Decisions Made
- Only `contract_tests.js` worth wiring into automated runs from sdk_test/
- SharedFixture pattern for read-only integration tests is the biggest perf win
- Single `test` script replacing fragmented scripts is the usability win

## Status
- [ ] Checklist created, plan under review
