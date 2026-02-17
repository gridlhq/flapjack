# Project Synopsis

**Last Updated:** 2026-02-13

## What Works (Production-Ready)

### ✅ Core Features Implemented
- **Overview Page:** Displays stats (indexes, docs, API calls), index list with pagination
- **Search & Browse:** Full-text search, facet filtering, pagination, JSON viewer
- **Settings:** Edit index settings (searchable attributes, facets, ranking, typo tolerance)
- **API Keys:** View keys list, create new keys, revoke keys (dialog UI)
- **Analytics:** 14 chart types showing search metrics, date range filtering

### ✅ UI/UX
- Dark mode toggle (persisted in localStorage)
- Responsive layout (mobile + desktop)
- Radix UI components (dialogs, dropdowns, tabs, accordions)
- Loading states and error handling
- API logger drawer for debugging

### ✅ Infrastructure
- Vite build system with TypeScript
- React Query for server state
- Zustand for global UI state
- React Hook Form + Zod validation
- TailwindCSS styling
- Playwright E2E tests (21/39 passing)

## What's In Progress (Partially Implemented)

### ⚠️ Testing Infrastructure
- **Unit tests:** None exist yet (need to add Vitest + RTL)
- **E2E tests:** 54% passing (21/39) - need backend running for full pass rate
- **Test organization:** Scattered across `tests/pages/` and `tests/integration/`
- **BDD specs:** Don't exist yet (need to write Tier 1 + Tier 2)

### ⚠️ Known Bugs
1. **Facets panel bug:** Makes redundant search query with `facetFilters` included - shows "No facets configured" when filtering produces 0 results. Root cause: `FacetsPanel.tsx:72-78`. Fix: lift search query to parent component.

2. **Analytics performance:** All 14 hooks fetch on mount instead of lazy loading per tab.

3. **Clear analytics UX:** Uses browser `confirm()` instead of proper ConfirmDialog component.

## What Doesn't Exist Yet (Planned)

### ❌ Missing Features
- Import index button (UI missing from Overview page)
- Export single index (only "Export All" exists)
- Settings validation (some invalid configs don't show errors)
- API key filtering/search (hard to find keys in long list)

### ❌ Missing Tests
- **Unit tests:** 0% coverage (need to write ~50-80 test files)
- **Smoke tests:** Not separated from full suite
- **Integration tests:** Limited coverage of multi-step flows
- **Test specs:** No Tier 2 detailed specs exist

### ❌ Missing Documentation
- BDD specifications (Tier 1 user stories)
- Test specifications (Tier 2 detailed specs)
- Component documentation
- Session handoffs

## Technical Debt

### High Priority
1. **No unit tests** - Blocks confident refactoring
2. **Facets panel architecture** - Needs state lifting
3. **Test organization** - Need smoke/full separation
4. **BDD documentation** - No behavior specs exist

### Medium Priority
1. **Analytics lazy loading** - Performance issue at scale
2. **Code splitting** - Recharts bundle is large (~50-60 KB)
3. **Error boundaries** - Only basic error handling exists
4. **Accessibility** - Not fully audited

### Low Priority
1. **Storybook** - Would help component development
2. **Visual regression tests** - No screenshot comparisons
3. **Performance budgets** - No Lighthouse assertions

## Current State Summary

**Works:** ✅ All major features implemented and functional
**Tests:** ⚠️ E2E tests exist but incomplete, no unit tests
**Docs:** ❌ No BDD specs, no test specs, minimal docs
**Bugs:** ⚠️ 3 known issues (facets, analytics, confirm dialog)

## Next Steps (Immediate)

1. **Set up Vitest + RTL** for unit testing
2. **Write BDD specifications** for all features
3. **Write test specs** (Tier 2) for all features
4. **Create unit tests** for components, hooks, utilities
5. **Reorganize E2E tests** into smoke/full suites
6. **Fix facets panel bug** (lift state)

## Metrics

- **Components:** ~50 components across `src/components/`, `src/pages/`
- **Custom hooks:** ~18 hooks in `src/hooks/`
- **API endpoints:** ~15 Flapjack API endpoints used
- **E2E tests:** 39 tests (21 passing, 18 failing)
- **Unit tests:** 0 (need to create)
- **Lines of code:** ~8,000 (TypeScript/TSX)

## Success Criteria

**Testing goals:**
- ✅ Unit tests: 85%+ coverage
- ✅ E2E smoke: 5-7 critical paths (~2 min)
- ✅ E2E full: 30-50 comprehensive tests (~10-15 min)
- ✅ All tests passing before any release

**Documentation goals:**
- ✅ BDD specs for all features
- ✅ Test specs for all E2E tests
- ✅ Session handoffs for all work
- ✅ Clear AI guidance in `_dev/` docs

This project's success depends entirely on automated testing. Without it, the single-maintainer model breaks down.
