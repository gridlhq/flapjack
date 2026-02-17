# SDK & Migration Tests

End-to-end tests that validate Flapjack's Algolia API compatibility against real Algolia.

**Prerequisites:** Algolia credentials in `.secret/.env.secret` (`ALGOLIA_APP_ID`, `ALGOLIA_ADMIN_KEY`) and Flapjack running on `localhost:7700`.

## Critical Tests

### `test_algolia_migration.js` — Algolia Migration (MOST IMPORTANT)

Proves a real customer can migrate from Algolia to Flapjack. Tests both migration paths:

- **Manual migration** (Phase 3/4): Export settings/synonyms/objects from Algolia, import into Flapjack via individual API calls, compare search results.
- **One-click migration** (Phase 3b/4b): Single `POST /1/migrate-from-algolia` call that does everything automatically, then compares search results.

```bash
node test_algolia_migration.js           # run full migration test
node test_algolia_migration.js --verbose # with detailed output
```

### `algolia_validation.js` — SDK Compatibility

Compares live Algolia responses against Flapjack using cached golden files. 15 test cases across 4 suites covering search, highlighting, filters, facets, and pagination.

```bash
node algolia_validation.js               # all tests with cache
node algolia_validation.js highlighting  # specific suite
node algolia_validation.js --no-cache    # force fresh API hits
node algolia_validation.js --verbose     # show detailed diffs
node algolia_validation.js --cleanup     # delete test indices
```

### `contract_tests.js` — API Contract Tests

Validates Flapjack API endpoint contracts (request/response shapes, status codes).

## Other Files

| File | Purpose |
|------|---------|
| `test_algolia_multi_pin.js` | Tests rules with multiple pin/hide operations |
| `test_exhaustive_fields.js` | Tests field type handling edge cases |
| `test_v4_simple.js` | Basic SDK v4 compatibility |
| `race_test.js` | Concurrent write/read race condition testing |
| `debug_search.js` | Manual search debugging utility |
| `audit_algolia_defaults.js` | Audits Algolia default settings |
| `MIGRATION_TEST_GUIDE.md` | Detailed guide for the migration test |
| `TEST_COVERAGE.md` | Validation test coverage matrix |

## One-Click Migration Endpoint

```
POST /1/migrate-from-algolia
{
  "appId": "YOUR_ALGOLIA_APP_ID",
  "apiKey": "YOUR_ALGOLIA_ADMIN_KEY",
  "sourceIndex": "products",
  "targetIndex": "products"   // optional, defaults to sourceIndex
}
```

Migrates an entire Algolia index (settings, synonyms, rules, all objects) into Flapjack in a single call. See `MIGRATION_TEST_GUIDE.md` for details.
