# E2E-UI Test Coverage Checklist

**Last updated:** 2026-02-14
**Total tests:** 196 across 16 spec files (7 smoke + 189 full)
**Test type:** NON-MOCKED, simulated-human, real-browser (Playwright + Chromium, headless)
**Runner:** `npm test` or `npx playwright test --project=e2e-ui`

---

## Test Categories — IMPORTANT DISTINCTION

| Category | Directory | What it tests | Browser? |
|----------|-----------|---------------|----------|
| **E2E-UI** | `tests/e2e-ui/` | Real browser + real server, simulated human clicks | YES (Chromium) |
| **E2E-API** | `tests/e2e-api/` | REST API calls against real server. **No browser. No `page.goto()`.** | NO (HTTP only) |

This checklist covers **E2E-UI tests only** — the non-mocked, real-browser tests.

---

## Per-Page Coverage

### Smoke Tests — [critical-paths.spec.ts](e2e-ui/smoke/critical-paths.spec.ts) (7 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Overview loads with real data | Done |
| 2 | Search returns real results | Done |
| 3 | Sidebar navigation works | Done |
| 4 | Settings page loads with searchable attributes | Done |
| 5 | API Keys page loads | Done |
| 6 | System health displays | Done |
| 7 | Create and delete index | Done |

### Overview Page — [overview.spec.ts](e2e-ui/full/overview.spec.ts) (16 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Index list shows e2e-products with document count (12) | Done |
| 2 | Stat cards: total indexes, documents, storage | Done |
| 3 | Health indicator shows Healthy | Done |
| 4 | Server health badge shows connected status | Done |
| 5 | Create new index, verify appears, then delete | Done |
| 6 | Create Index dialog shows template options | Done |
| 7 | Selecting Movies template auto-fills index name | Done |
| 8 | Export All and Upload buttons visible | Done |
| 9 | Per-index export and import buttons visible | Done |
| 10 | Index row shows storage size and update info | Done |
| 11 | Analytics summary section displays data | Done |
| 12 | Analytics chart renders in overview analytics section | Done |
| 13 | View Details link navigates to analytics page | Done |
| 14 | Settings link navigates to settings page | Done |
| 15 | Clicking index navigates to search page | Done |
| 16 | Export All button triggers download | Done |

### Search & Browse — [search.spec.ts](e2e-ui/full/search.spec.ts) (22 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Search by typing and pressing Enter | Done |
| 2 | Search by clicking Search button | Done |
| 3 | Search results display with hit count | Done |
| 4 | Facet panel displays with category/brand facets | Done |
| 5 | Click facet value filters results | Done |
| 6 | Clear filters restores original results | Done |
| 7 | Multiple facet filters combine correctly | Done |
| 8 | Facets panel persists when filter produces 0 results | Done |
| 9 | Add Documents dialog opens and closes | Done |
| 10 | Sample Data tab loads movie/product data | Done |
| 11 | Pagination controls appear when results exceed one page | Done |
| 12 | Empty query shows all documents | Done |
| 13 | No results message for nonsense query | Done |
| 14 | Analytics toggle switch | Done |
| 15 | Typo tolerance returns results for misspelled queries | Done |
| 16 | Different searches return distinct result sets | Done |
| 17 | Synonym "screen" returns monitor results | Done |
| 18 | Synonym "earbuds" returns headphone results | Done |
| 19 | Synonym "notebook" returns laptop results | Done |
| 20 | Sub-page nav buttons (Settings, Synonyms, etc.) | Done |
| 21 | Index name breadcrumb and stats | Done |
| 22 | Facet values show document counts | Done |

### Analytics — [analytics.spec.ts](e2e-ui/full/analytics.spec.ts) (19 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Overview tab loads with KPI cards showing data | Done |
| 2 | Search volume chart is visible on Overview tab | Done |
| 3 | Top searches table shows data on Overview tab | Done |
| 4 | No-Result Rate Over Time chart is visible | Done |
| 5 | No Results tab loads with rate banner and table | Done |
| 6 | Devices tab shows platform breakdown | Done |
| 7 | Geography tab shows country list | Done |
| 8 | Geography drill-down: click country, see details, back | Done |
| 9 | Date range toggle (7d/30d/90d) triggers API calls | Done |
| 10 | Searches tab shows top searches table with data | Done |
| 11 | Searches tab filter input narrows results | Done |
| 12 | Searches tab shows country filter dropdown | Done |
| 13 | Searches tab shows device filter dropdown | Done |
| 14 | Searches tab column headers clickable for sorting | Done |
| 15 | Filters tab shows Top Filter Attributes table | Done |
| 16 | Filters tab: expand/collapse filter rows | Done |
| 17 | Flush button is visible and clickable | Done |
| 18 | Analytics page shows BETA badge | Done |
| 19 | Clear Analytics button opens confirmation dialog | Done |

### Analytics Deep Data — [analytics-deep.spec.ts](e2e-ui/full/analytics-deep.spec.ts) (15 tests)

| # | Test | Status |
|---|------|--------|
| 1 | KPI cards show non-zero numeric values from seeded data | Done |
| 2 | Search volume chart renders SVG with data path | Done |
| 3 | Top 10 searches table shows ranked queries descending | Done |
| 4 | KPI cards show delta comparison badges | Done |
| 5 | Searches tab displays sortable table in descending order | Done |
| 6 | Searches tab text filter narrows results client-side | Done |
| 7 | No Results tab shows rate banner (0-100%) | Done |
| 8 | No Results tab shows zero-result queries table | Done |
| 9 | Devices tab shows platform cards (desktop > mobile) | Done |
| 10 | Devices tab shows device chart with SVG rendering | Done |
| 11 | Geography tab shows country table with US as top | Done |
| 12 | Geography country percentages sum to ~100% | Done |
| 13 | Geography click country shows drill-down | Done |
| 14 | Geography back button returns to country list | Done |
| 15 | Switching to 30d updates KPI values | Done |

### Rules — [rules.spec.ts](e2e-ui/full/rules.spec.ts) (12 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Rules page loads with seeded rules | Done |
| 2 | Rule cards show pattern and actions | Done |
| 3 | Rule badges (pin/hide) display | Done |
| 4 | Rules count badge shows correct number | Done |
| 5 | Search input filters rules | Done |
| 6 | Add Rule button opens dialog | Done |
| 7 | Merchandising Studio link navigates | Done |
| 8 | Rule card structure (ID, pattern, description) | Done |
| 9 | Delete rule via API + UI verification | Done |
| 10 | Create rule via Add Rule dialog (JSON editor) | Done |
| 11 | Delete rule via UI confirm dialog | Done |
| 12 | Clear All rules button + cancel | Done |

### Synonyms — [synonyms.spec.ts](e2e-ui/full/synonyms.spec.ts) (10 tests)

| # | Test | Status |
|---|------|--------|
| 1 | List shows seeded synonyms | Done |
| 2 | Synonym type badges (Multi-way) | Done |
| 3 | Synonym count badge | Done |
| 4 | Create and delete multi-way synonym | Done |
| 5 | Create one-way synonym via dialog | Done |
| 6 | Search/filter synonyms | Done |
| 7 | Add Synonym button opens dialog | Done |
| 8 | Synonym card structure (equals-joined words) | Done |
| 9 | Delete synonym via API + UI verification | Done |
| 10 | Clear All button shows confirmation (cancel) | Done |

### Settings — [settings.spec.ts](e2e-ui/full/settings.spec.ts) (11 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Searchable attributes from seeded settings | Done |
| 2 | Faceting attributes display | Done |
| 3 | JSON editor toggle | Done |
| 4 | Ranking/custom ranking configuration | Done |
| 5 | Compact index button visible and enabled | Done |
| 6 | Compact index button triggers compaction | Done |
| 7 | FilterOnly faceting attributes | Done |
| 8 | Breadcrumb back to index | Done |
| 9 | All major sections present | Done |
| 10 | Reset button appears after modification and reverts | Done |
| 11 | Save settings + verify persistence | Done |

### Merchandising — [merchandising.spec.ts](e2e-ui/full/merchandising.spec.ts) (10 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Search for products shows results | Done |
| 2 | Pin button visible on result cards | Done |
| 3 | Hide button visible on result cards | Done |
| 4 | Pinning shows badge + moves to position 1 | Done |
| 5 | Hiding moves result to hidden section | Done |
| 6 | Pin + hide multiple results (combined counts) | Done |
| 7 | Save as rule → cross-page verify on Rules page | Done |
| 8 | Different queries return different results | Done |
| 9 | Results summary shows hit count | Done |
| 10 | How It Works help card visible | Done |

### API Keys — [api-keys.spec.ts](e2e-ui/full/api-keys.spec.ts) (10 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Key list loads (keys or empty state) | Done |
| 2 | Create new API key via dialog | Done |
| 3 | Create then delete API key | Done |
| 4 | Key permissions badges display | Done |
| 5 | Copy button visible on key cards | Done |
| 6 | Copy button click shows "Copied" feedback | Done |
| 7 | Create key dialog shows all form sections | Done |
| 8 | Toggle permissions updates badges | Done |
| 9 | Key with no index scope shows "All Indexes" | Done |
| 10 | Create key with index scope restriction | Done |

### Search Logs — [search-logs.spec.ts](e2e-ui/full/search-logs.spec.ts) (11 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Log entries appear after visiting pages | Done |
| 2 | Log entry shows HTTP method and URL | Done |
| 3 | Expand log entry shows curl command and status | Done |
| 4 | Collapse expanded log entry | Done |
| 5 | Clear Logs removes entries and shows empty state | Done |
| 6 | Filter input narrows log entries by URL | Done |
| 7 | View mode toggle (Endpoint ↔ Curl) | Done |
| 8 | Curl view shows actual curl commands with correct format | Done |
| 9 | Expanded log entry shows request body and response | Done |
| 10 | Export button visible | Done |
| 11 | Request count badge shows accurate count | Done |

### System — [system.spec.ts](e2e-ui/full/system.spec.ts) (16 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Health tab: server status "ok" | Done |
| 2 | Health tab: active writers count | Done |
| 3 | Health tab: facet cache with numeric values | Done |
| 4 | Health tab: index health summary + green dots | Done |
| 5 | Health tab: auto-refresh notice | Done |
| 6 | Indexes tab: e2e-products with doc count | Done |
| 7 | Indexes tab: total indexes/docs/storage cards | Done |
| 8 | Indexes tab: health status column (Healthy) | Done |
| 9 | Indexes tab: click index → search page | Done |
| 10 | Replication tab: Node ID card | Done |
| 11 | Replication tab: Enabled/Disabled status | Done |
| 12 | Replication tab: auto-refresh notice | Done |
| 13 | Snapshots tab: Local Export/Import section | Done |
| 14 | Snapshots tab: per-index export/import buttons | Done |
| 15 | Snapshots tab: S3 Backups section | Done |
| 16 | All four tabs visible + clickable | Done |

### Migrate — [migrate.spec.ts](e2e-ui/full/migrate.spec.ts) (13 tests)

| # | Test | Status |
|---|------|--------|
| 1 | All form sections visible on load | Done |
| 2 | Migrate button disabled when empty | Done |
| 3 | Filling credentials enables button | Done |
| 4 | API key visibility toggle (eye button) | Done |
| 5 | Overwrite toggle on/off | Done |
| 6 | Target index placeholder mirrors source | Done |
| 7 | Custom target overrides source in button | Done |
| 8 | Clearing source re-disables button | Done |
| 9 | Clearing app ID re-disables button | Done |
| 10 | Invalid credentials shows error | Done |
| 11 | Info section content (3 items) | Done |
| 12 | Target field helper text | Done |
| 13 | API key security note | Done |

### Migrate (Algolia) — [migrate-algolia.spec.ts](e2e-ui/full/migrate-algolia.spec.ts) (2 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Full Algolia migration: fill form → migrate → verify success → browse | Done |
| 2 | Invalid Algolia credentials show error state | Done |

*Note: Skipped when Algolia credentials not available.*

### Navigation & Layout — [navigation.spec.ts](e2e-ui/full/navigation.spec.ts) (14 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Sidebar shows all main nav items | Done |
| 2 | Sidebar shows seeded index | Done |
| 3 | Click Overview → overview page | Done |
| 4 | Click API Logs → logs page | Done |
| 5 | Click Migrate → migrate page | Done |
| 6 | Click API Keys → keys page | Done |
| 7 | Click System → system page | Done |
| 8 | Click index → search page | Done |
| 9 | Header shows logo + connection status | Done |
| 10 | Theme toggle light/dark | Done |
| 11 | Indexing queue button opens panel | Done |
| 12 | Search sub-page nav buttons | Done |
| 13 | Breadcrumb navigates to overview | Done |
| 14 | Unknown route shows 404 | Done |

### Cross-Page Flows — [cross-page-flows.spec.ts](e2e-ui/full/cross-page-flows.spec.ts) (8 tests)

| # | Test | Status |
|---|------|--------|
| 1 | Overview → click index → Search page | Done |
| 2 | Full lifecycle: create → docs → search → delete | Done |
| 3 | Merchandising → pin → save rule → Rules page | Done |
| 4 | System Indexes tab → click → search page | Done |
| 5 | Settings persistence after save + reload | Done |
| 6 | Search with analytics → Analytics page | Done |
| 7 | Overview analytics → Analytics page link | Done |
| 8 | Full navigation cycle (5 pages) | Done |

---

## Summary

| Page/Feature | File | Tests | Coverage |
|-------------|------|-------|----------|
| Smoke (critical paths) | critical-paths.spec.ts | 7 | Full |
| Overview | overview.spec.ts | 16 | Full |
| Search & Browse | search.spec.ts | 22 | Full |
| Analytics | analytics.spec.ts | 19 | Full |
| Analytics Deep Data | analytics-deep.spec.ts | 15 | Full |
| Rules | rules.spec.ts | 12 | Full |
| Synonyms | synonyms.spec.ts | 10 | Full |
| Settings | settings.spec.ts | 11 | Full |
| Merchandising | merchandising.spec.ts | 10 | Full |
| API Keys | api-keys.spec.ts | 10 | Full |
| Search Logs | search-logs.spec.ts | 11 | Full |
| System | system.spec.ts | 16 | Full |
| Migrate | migrate.spec.ts | 13 | Full |
| Migrate (Algolia) | migrate-algolia.spec.ts | 2 | Full |
| Navigation/Layout | navigation.spec.ts | 14 | Full |
| Cross-Page Flows | cross-page-flows.spec.ts | 8 | Full |
| **TOTAL** | **16 files** | **196** | **Full** |

---

## Quality Standards

- **Zero conditional assertions** — no `if (await isVisible())` guards that silently pass
- **Zero sleeps** — all waits use Playwright auto-retry (`expect().toBeVisible()`, `waitForResponse()`)
- **Real server** — every test runs against a live Flapjack backend with seeded data
- **Real browser** — Chromium via Playwright (headless mode for CI/local)
- **Simulated human** — all interactions use getByRole/getByText/getByTestId locators
- **Deterministic data** — 12 products, 3 synonyms, 2 rules, settings seeded via seed.setup.ts
- **Cleanup** — tests that create data clean up after themselves (API or UI)

---

## Running Tests

```bash
cd engine/dashboard

# Run all E2E-UI tests (headless, default)
npm test

# Run smoke tests only (~2 min)
npm run test:e2e-ui:smoke

# Run a specific test file
npx playwright test tests/e2e-ui/full/overview.spec.ts

# Run E2E-API tests (no browser)
npm run test:e2e-api

# Show HTML report after run
npx playwright show-report
```

---

## Seed Data Reference

From `tests/fixtures/test-data.ts`:

- **12 products** (p01-p12): Laptops, Tablets, Audio, Storage, Monitors, Accessories
- **9 brands**: Apple, Lenovo, Dell, Samsung, Sony, LG, Logitech, Keychron, CalDigit
- **3 synonyms**: laptop/notebook/computer, headphones/earphones/earbuds, monitor/screen/display
- **2 rules**: Pin MacBook Pro for "laptop", Hide Galaxy Tab for "tablet"
- **Settings**: 5 searchable attributes, 4 faceting attributes, 2 custom ranking rules
- **Analytics**: 7 days of search/click/geo/device data seeded via `/2/analytics/seed`
