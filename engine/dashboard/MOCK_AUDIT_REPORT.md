# Mock Usage Audit Report
## Dashboard E2E Tests

**Date:** 2026-02-13
**Purpose:** Audit all test files for mock usage and recommend conversion to real E2E tests

---

## Executive Summary

- **Total test files audited:** 15
- **Files using mocks:** 13 (87%)
- **Files with real E2E:** 2 (13%)
  - ✅ `analytics.spec.ts` - Real E2E with backend seed
  - ✅ `search-logs.spec.ts` - Real E2E

**Total mock references:** 387 across 13 files

---

## Files Ranked by Mock Usage

| File | Mock Count | Status | Priority |
|------|------------|--------|----------|
| `search.spec.ts` | 70 | ❌ Heavily mocked | HIGH |
| `settings.spec.ts` | 47 | ❌ Heavily mocked | HIGH |
| `apikeys.spec.ts` | 43 | ❌ Heavily mocked | MEDIUM |
| `system.spec.ts` | 34 | ❌ Heavily mocked | MEDIUM |
| `merchandising.spec.ts` | 30 | ❌ Heavily mocked | HIGH |
| `navigation.spec.ts` | 25 | ❌ Heavily mocked | LOW |
| `snapshots.spec.ts` | 25 | ❌ Heavily mocked | MEDIUM |
| `overview.spec.ts` | 24 | ❌ Heavily mocked | HIGH |
| `rules.spec.ts` | 24 | ❌ Heavily mocked | HIGH |
| `synonyms.spec.ts` | 22 | ❌ Heavily mocked | HIGH |
| `multi-index-ui.spec.ts` | 18 | ❌ Heavily mocked | MEDIUM |
| `migrate.spec.ts` | 16 | ❌ Heavily mocked | LOW |
| `multi-index.spec.ts` | 9 | ❌ Mocked | LOW |
| `analytics.spec.ts` | 0 | ✅ Real E2E | ✅ DONE |
| `search-logs.spec.ts` | 0 | ✅ Real E2E | ✅ DONE |

---

## Common Mock Patterns Observed

### 1. **API Route Mocking**
```typescript
await page.route('**/1/indexes/*/query', (route) => {
  route.fulfill({
    status: 200,
    contentType: 'application/json',
    body: JSON.stringify(MOCK_DATA)
  });
});
```

**Issues:**
- Tests pass with fake data
- No verification of real backend behavior
- Mocks can drift from actual API responses
- No integration testing

### 2. **Hardcoded Mock Data**
```typescript
const MOCK_SEARCH = {
  hits: [...],
  nbHits: 3,
  // ... hardcoded responses
};
```

**Issues:**
- Data doesn't represent real scenarios
- No edge cases (empty states, errors)
- Tests don't catch backend breaking changes

### 3. **Multiple API Mocks Per Test**
Most tests mock 3-5 different endpoints simultaneously, creating complex mock setups that are:
- Hard to maintain
- Fragile (break when API changes)
- Give false confidence

---

## Recommended Conversion Strategy

### Phase 1: High-Priority Features (Core Search Functionality)
**Files:** `search.spec.ts`, `merchandising.spec.ts`, `overview.spec.ts`, `rules.spec.ts`, `synonyms.spec.ts`, `settings.spec.ts`

**Approach:**
1. Create real test indices with sample data
2. Use backend seed functions where available
3. Test real search queries, faceting, ranking
4. Verify UI rendering of real results

**Estimated effort:** 3-5 days

### Phase 2: Medium-Priority (Admin Features)
**Files:** `apikeys.spec.ts`, `system.spec.ts`, `snapshots.spec.ts`, `multi-index-ui.spec.ts`

**Approach:**
1. Create real API keys via backend
2. Test real snapshot creation/restore
3. Verify system stats from real backend
4. Test multi-index management with real indices

**Estimated effort:** 2-3 days

### Phase 3: Low-Priority (Navigation, Misc)
**Files:** `navigation.spec.ts`, `migrate.spec.ts`, `multi-index.spec.ts`

**Approach:**
1. Convert to lightweight integration tests
2. Focus on critical paths only
3. May retain some mocks for edge cases

**Estimated effort:** 1-2 days

---

## Benefits of Real E2E Tests

### ✅ Proven with Analytics Tests
The `analytics.spec.ts` conversion demonstrated:

**Before (mocked):**
- 100% pass rate with fake data
- 0 bugs caught
- False confidence

**After (real E2E):**
- Caught geography data issues
- Found chart rendering problems
- Discovered backend seed timing issues
- Tests now verify REAL user flows

**Result:** 23/25 tests passing with real data = **92% pass rate with actual confidence**

### Key Advantages
1. **Real bug detection** - Finds integration issues
2. **Backend verification** - Ensures API contracts are met
3. **Data validation** - Tests work with real data shapes
4. **Regression prevention** - Catches breaking changes
5. **True E2E coverage** - UI → API → Database → UI

---

## Implementation Guidelines

### 1. Test Data Strategy
```typescript
// ✅ GOOD: Use backend seed functions
await request.post(`${API}/2/analytics/seed`, {
  data: { index: 'test-index', days: 7 }
});

// ❌ BAD: Mock API responses
await page.route('**/analytics', (route) => {
  route.fulfill({ body: JSON.stringify(FAKE_DATA) });
});
```

### 2. Setup/Teardown Pattern
```typescript
test.beforeAll(async ({ request }) => {
  // Create real test data
  await seedTestData(request, indexName);
});

test.afterAll(async ({ request }) => {
  // Clean up
  await deleteIndex(request, indexName);
});
```

### 3. Wait for Real Data
```typescript
// ✅ GOOD: Wait for real data to be available
await expect(async () => {
  const response = await request.get(`/api/data`);
  expect(response.status()).toBe(200);
}).toPass({ timeout: 10000 });

// ❌ BAD: Assume mock data is instant
await page.route('**/api/data', ...);
await page.goto('/page'); // No verification needed
```

---

## Next Steps

1. ✅ **Analytics tests converted** (COMPLETED)
2. **Create test data fixtures**
   - Search index seeds
   - Synonym/rule templates
   - API key generators
3. **Convert search.spec.ts** (highest priority, 70 mocks)
4. **Convert settings.spec.ts** (47 mocks)
5. **Convert merchandising.spec.ts** (30 mocks)
6. **Continue through priority list**

---

## Metrics Goal

**Current state:**
- 13/15 files mocked (87%)
- 387 total mock references
- False confidence

**Target state:**
- 0/15 files mocked (0%)
- Real E2E coverage
- True confidence in deployments

**Timeline:** 6-10 days for full conversion
