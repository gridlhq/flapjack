# Feature Status

**Last Updated:** 2026-02-13

## Feature Priority Matrix

| Priority | Feature | Status | Tests | Notes |
|----------|---------|--------|-------|-------|
| P0 | Testing Infrastructure | 🟡 In Progress | ⚠️ 54% | Need unit tests, BDD specs |
| P0 | Fix Facets Panel Bug | 🔴 Bug | ⚠️ Partial | Redundant queries, state lifting needed |
| P1 | Overview Page | ✅ Complete | ⚠️ 100% E2E | Need unit tests |
| P1 | Search & Browse | ✅ Complete | ⚠️ 44% E2E | Need unit tests |
| P1 | Settings Page | ✅ Complete | ⚠️ 38% E2E | Need unit tests, validation |
| P1 | API Keys | ✅ Complete | ⚠️ 14% E2E | Need unit tests |
| P1 | Analytics | ✅ Complete | ⚠️ 0% | Need all tests, fix lazy loading |
| P2 | Import Index | 🔴 Missing | ❌ None | UI button + flow needed |
| P2 | Clear Analytics UX | 🟡 Partial | ❌ None | Replace confirm() with dialog |
| P3 | Export Single Index | 🔴 Missing | ❌ None | Only "Export All" exists |
| P3 | Dark Mode | ✅ Complete | ⚠️ Partial | Selector mismatch in tests |

## Status Legend
- ✅ **Complete** - Fully implemented, working as expected
- 🟡 **Partial** - Implemented but has issues or missing pieces
- 🔴 **Missing** - Not implemented yet
- ⚠️ **Bug** - Implemented but has known bugs

## Test Coverage Legend
- ✅ **Complete** - Unit + E2E tests passing
- ⚠️ **Partial** - Some tests exist, incomplete coverage
- ❌ **None** - No tests written

---

## P0: Critical (Must Fix Now)

### Testing Infrastructure
**Status:** 🟡 In Progress | **Tests:** ⚠️ 54% passing

**What exists:**
- Playwright E2E tests (39 tests, 21 passing)
- Test fixtures with sample data
- Global setup for tests

**What's missing:**
- Unit tests (Vitest + RTL not set up)
- BDD specifications (Tier 1)
- Test specifications (Tier 2)
- Smoke/full test separation

**Next steps:**
1. Install Vitest + React Testing Library
2. Write BDD specifications for all features
3. Write test specifications (Tier 2)
4. Create unit tests (85%+ coverage)
5. Reorganize E2E tests into smoke/full

---

### Fix Facets Panel Bug
**Status:** 🔴 Bug | **Tests:** ⚠️ Partial

**Problem:**
FacetsPanel makes its own search query with `facetFilters` included. When filtering produces 0 results, shows "No facets configured" instead of "No results match current filters".

**Root cause:**
- File: `src/components/search/FacetsPanel.tsx:72-78`
- Component makes independent search query
- Doesn't share state with SearchBrowse parent

**Solution:**
Lift search query to SearchBrowse parent, pass facets data down to FacetsPanel.

**Test spec needed:**
`tests/specs/search-browse.md` should cover:
1. Search with facet filter → verify facets still show
2. Filter that produces 0 results → verify message says "No results" not "No facets configured"

---

## P1: High Priority (Core Features)

### Overview Page
**Status:** ✅ Complete | **Tests:** ⚠️ 100% E2E, 0% Unit

**Features:**
- Stats cards (4 metrics: indexes, documents, API calls, storage)
- Index list with search/filter
- Pagination controls
- Browse/Settings/Delete actions per index
- Create Index button

**Test coverage:**
- ✅ E2E: 7/7 tests passing
- ❌ Unit: 0 tests

**Needs:**
- Unit tests for OverviewStats component
- Unit tests for IndexList component
- Unit tests for IndexCard component
- Test spec in `tests/specs/index-management.md`

---

### Search & Browse
**Status:** ✅ Complete | **Tests:** ⚠️ 44% E2E, 0% Unit

**Features:**
- Search input with Enter key support
- Document cards with JSON viewer (expand/collapse)
- Facets panel (with known bug - see P0)
- Pagination
- Results count

**Test coverage:**
- ⚠️ E2E: 4/9 tests passing (need backend for full pass)
- ❌ Unit: 0 tests

**Needs:**
- Fix facets panel bug (P0)
- Unit tests for SearchBox component
- Unit tests for DocumentCard component
- Unit tests for FacetsPanel component
- Unit tests for ResultsPanel component
- Test spec in `tests/specs/search-browse.md`

---

### Settings Page
**Status:** ✅ Complete | **Tests:** ⚠️ 38% E2E, 0% Unit

**Features:**
- Edit searchable attributes
- Configure faceting attributes
- Set custom ranking
- Typo tolerance settings
- Save/Reset buttons

**Test coverage:**
- ⚠️ E2E: 3/8 tests passing (need backend for full pass)
- ❌ Unit: 0 tests

**Needs:**
- Better form validation (some invalid configs accepted)
- Unit tests for SettingsForm component
- Unit tests for form validation logic
- Test spec in `tests/specs/settings-form.md`

---

### API Keys
**Status:** ✅ Complete | **Tests:** ⚠️ 14% E2E, 0% Unit

**Features:**
- Keys list (read-only display)
- Create key dialog
- Revoke key action
- Copy key to clipboard

**Test coverage:**
- ⚠️ E2E: 1/7 tests passing (need backend for full pass)
- ❌ Unit: 0 tests

**Needs:**
- Key filtering/search for long lists
- Unit tests for KeysList component
- Unit tests for CreateKeyDialog component
- Test spec in `tests/specs/api-keys.md`

---

### Analytics
**Status:** ✅ Complete | **Tests:** ❌ 0%

**Features:**
- 14 chart types (queries, latency, results, filters, etc.)
- Date range filtering
- Tab-based navigation
- Recharts visualizations

**Test coverage:**
- ❌ E2E: 0 tests
- ❌ Unit: 0 tests

**Needs:**
- Fix lazy loading (all 14 hooks fetch on mount)
- Unit tests for Analytics components
- Unit tests for useAnalytics* hooks
- E2E smoke test for analytics page load
- Test spec in `tests/specs/analytics.md`

---

## P2: Medium Priority (UX Improvements)

### Import Index
**Status:** 🔴 Missing | **Tests:** ❌ None

**What's needed:**
- "Import Index" button on Overview page (next to "Export All")
- Dialog with file picker (JSON file)
- Validation of JSON format
- Progress indicator during import
- Success/error toast

**Test plan:**
1. Click import → file picker opens
2. Select valid JSON → import succeeds → index appears in list
3. Select invalid JSON → error message shown
4. Cancel import → dialog closes, no changes

---

### Clear Analytics UX
**Status:** 🟡 Partial | **Tests:** ❌ None

**Problem:**
Uses browser `confirm()` instead of proper ConfirmDialog component.

**Location:**
`src/pages/Analytics.tsx:148`

**Fix:**
1. Create ConfirmDialog component (or use Radix AlertDialog)
2. Replace confirm() call
3. Use `queryClient.resetQueries()` so data visually clears without page refresh

**Test plan:**
1. Click "Clear Analytics" → dialog appears
2. Click "Cancel" → dialog closes, no change
3. Click "Confirm" → analytics data clears, UI updates

---

## P3: Low Priority (Nice-to-Have)

### Export Single Index
**Status:** 🔴 Missing | **Tests:** ❌ None

**What exists:**
"Export All" button exports all indexes as JSON.

**What's missing:**
Export button per individual index (in index card actions).

**Test plan:**
1. Click export on specific index → download JSON file
2. Verify JSON contains only that index's documents
3. Verify settings are included in export

---

### Dark Mode
**Status:** ✅ Complete | **Tests:** ⚠️ Partial

**Features:**
- Toggle button in header
- Persists in localStorage
- Applies to all pages

**Issue:**
E2E test selector doesn't match button (expects "toggle theme", actual might differ).

**Fix:**
Add `aria-label="toggle theme"` to dark mode button or use `data-testid`.

---

## Feature Roadmap (Post-MVP)

### Future Features (Not Prioritized)
- Bulk document operations
- Query rules UI
- Synonyms management
- Advanced filtering (date ranges, numeric ranges)
- Search analytics drill-down (per-query details)
- User preferences (default index, page size)
- Keyboard shortcuts
- Command palette

### Infrastructure Improvements
- Storybook for component documentation
- Visual regression tests (screenshot comparisons)
- Accessibility audit (axe-core integration)
- Performance budgets (Lighthouse assertions)
- Error boundaries for all major sections
- Code splitting for Recharts bundle

---

## Notes

**Testing is the blocker.** All features work, but without comprehensive tests, we can't confidently:
- Refactor code
- Add new features
- Fix bugs
- Accept contributions

**Priority order:**
1. Set up unit testing (P0)
2. Write BDD specs (P0)
3. Fix facets bug (P0)
4. Add missing tests for existing features (P1)
5. Implement missing features (P2-P3)

**Don't add new features until testing infrastructure is solid.**
