# Test Specification: Search & Browse (Tier 2)

**Feature:** Search documents with facet filtering
**BDD Spec:** B-SEARCH-001, B-SEARCH-002, B-SEARCH-003, B-SEARCH-004
**Priority:** P1
**Last Updated:** 2026-02-13

---

## Test Fixtures

**Location:** `tests/fixtures/test-data.ts`

```typescript
// Actual seed data: 12 products in tests/fixtures/test-data.ts
// 3 laptops: MacBook Pro 16", ThinkPad X1 Carbon, Dell XPS 15
export const searchFixtures = {
  query: {
    simple: 'laptop',
    withTypo: 'labtop', // should match 'laptop'
    noResults: 'xyz123abc',
    special: 'c++',
  },
  expectedResults: {
    simple: {
      count: 3, // 3 laptops in seed data
      firstTitle: 'MacBook Pro 16"', // highest rating (4.8) due to desc(rating) ranking
    },
    withTypo: {
      count: 3, // same as 'laptop' due to typo tolerance
    },
    noResults: {
      count: 0,
    },
  },
  facets: {
    brand: ['Apple', 'Lenovo', 'Dell', 'Samsung', 'Sony', 'LG', 'Logitech', 'Keychron', 'CalDigit'],
    category: ['Laptops', 'Tablets', 'Audio', 'Storage', 'Monitors', 'Accessories'],
  },
}
```

---

## TEST: Search with Enter key

**User Story:** B-SEARCH-001
**Type:** E2E Smoke
**File:** `tests/e2e-ui/smoke/search.spec.ts`

### Setup
- Index `products` exists with sample documents
- Navigate to `/search/products`

### Execute
1. Wait for page load
2. Focus search input `[data-testid="search-input"]`
3. Type `searchFixtures.query.simple` ('laptop')
4. Press Enter key

### Verify UI
- `[data-testid="results-panel"]` is visible
- `[data-testid="document-card"]` count >= 1
- First card contains text matching `searchFixtures.expectedResults.simple.firstTitle`
- Results count shows "3 results"

### Verify API
- GET `/indexes/products/search?q=laptop` returns 200
- Response.hits.length = 3
- Response.hits[0].title contains 'MacBook'

### Expected Values
- Total results: 3 (from fixtures.expectedResults.simple.count)
- First title: 'MacBook Pro 16"' (from fixtures)

### Cleanup
- None (read-only)

---

## TEST: Search with typo tolerance

**User Story:** B-SEARCH-001 (edge case)
**Type:** E2E Full
**File:** `tests/e2e-ui/full/search.spec.ts`

### Fixtures
- Query with typo: `searchFixtures.query.withTypo` ('labtop')
- Expected results: same as 'laptop'

### Execute
1. Navigate to `/search/products`
2. Type 'labtop' (typo)
3. Press Enter

### Verify UI
- Results panel shows results (not "No results")
- Results count: "3 results"
- First card matches "MacBook Pro 16""

### Verify API
- Response includes `_rankingInfo` with `typoCorrection` field
- Typo distance <= 1

### Expected Values
- Same results as searching 'laptop'
- Count: 3

### Cleanup
- None

---

## TEST: Filter by single facet

**User Story:** B-SEARCH-002
**Type:** E2E Full
**File:** `tests/e2e-ui/full/search-facets.spec.ts`

### Setup
- Index `products` with facets configured: ['brand', 'category']
- Navigate to `/search/products`
- Search for 'laptop'

### Execute
1. Wait for results to load
2. Locate `[data-testid="facets-panel"]`
3. Find facet value "Apple" under "brand" facet
4. Click facet value button

### Verify UI
- Active filter chip shows "brand: Apple"
- Results panel updates immediately
- All document cards show "Apple" brand
- Facets panel still visible (NOT "No facets configured")
- Other facet values still shown with updated counts

### Verify API
- GET `/indexes/products/search?q=laptop&facetFilters=["brand:Apple"]`
- Response.hits all have brand: 'Apple'
- Response.facets.brand still present with counts

### Expected Values
- Filtered results count: <= original count
- All results have brand = 'Apple'

### Cleanup
- None

---

## TEST: Filter by facet producing 0 results

**User Story:** B-SEARCH-002-BUG (regression test)
**Type:** E2E Full
**File:** `tests/e2e-ui/full/search-facets.spec.ts`

### Setup
- Index with documents that don't match specific filter combination
- Search for 'laptop', then apply incompatible facets

### Execute
1. Navigate to `/search/products`
2. Search for 'laptop'
3. Click facet "brand: Apple"
4. Click facet "category: Gaming" (assuming no Apple gaming laptops exist)

### Verify UI
- Results panel shows "No results match current filters"
- **Facets panel still shows facets** (NOT "No facets configured")
- Active filters show both: "brand: Apple" AND "category: Gaming"
- Clear filters button is visible

### Verify API
- Search request returns 0 hits
- Response.facets still present (facets should always be returned)

### Expected Values
- Results count: 0
- Facets panel: visible with facet values
- Error message: "No results match current filters"
- **NOT:** "No facets configured"

### Cleanup
- None

**Note:** This is a regression test for the facets panel bug (B-SEARCH-002-BUG)

---

## TEST: Clear facet filters

**User Story:** B-SEARCH-002
**Type:** E2E Full
**File:** `tests/e2e-ui/full/search-facets.spec.ts`

### Setup
- Search with active facet filters

### Execute
1. Navigate to `/search/products?q=laptop`
2. Apply facet filter "brand: Apple"
3. Verify filtered results
4. Click "Clear filters" button

### Verify UI
- Active filter chips disappear
- Results panel shows original unfiltered results
- Results count returns to original count
- Facet counts update to original values

### Verify API
- GET `/indexes/products/search?q=laptop` (no facetFilters)
- Response matches original search

### Expected Values
- Results count: back to original (3)
- No active filters

### Cleanup
- None

---

## TEST: Paginate search results

**User Story:** B-SEARCH-003
**Type:** E2E Full
**File:** `tests/e2e-ui/full/search-pagination.spec.ts`

### Setup
- Index with 25 documents (more than one page at 10 per page)
- Search returns all 25 documents

### Execute
1. Navigate to `/search/products?q=laptop`
2. Wait for results to load
3. Verify page 1 shows 10 results
4. Click "Next" button
5. Wait for page 2 to load

### Verify UI
- Page 1: shows documents 1-10
- Page 2: shows documents 11-20
- "Previous" button enabled on page 2
- "Next" button disabled on last page
- URL updates to include page parameter: `?q=laptop&page=2`
- Search query and filters maintained

### Verify API
- Page 1: GET `/search?q=laptop&page=0&hitsPerPage=10`
- Page 2: GET `/search?q=laptop&page=1&hitsPerPage=10`

### Expected Values
- Total hits: 25
- Total pages: 3
- Page 1 hits: 10
- Page 2 hits: 10
- Page 3 hits: 5

### Cleanup
- None

---

## TEST: Expand document to view JSON

**User Story:** B-SEARCH-004
**Type:** E2E Full
**File:** `tests/e2e-ui/full/document-viewer.spec.ts`

### Setup
- Search with results

### Execute
1. Navigate to `/search/products?q=laptop`
2. Find first `[data-testid="document-card"]`
3. Verify collapsed state shows only summary
4. Click expand button
5. Verify JSON viewer appears

### Verify UI
- Collapsed: shows only title, snippet, objectID
- Expanded: shows full JSON with syntax highlighting
- Expand/collapse toggles independently per card
- Multiple cards can be expanded simultaneously
- JSON is formatted and readable

### Expected Values
- JSON structure matches document schema
- All fields visible in expanded view

### Cleanup
- None

---

## Unit Test Specifications

### Component: SearchBox

**File:** `src/components/search/SearchBox.test.tsx`

#### Test: Calls onSearch when Enter pressed
```typescript
const handleSearch = vi.fn()

render(<SearchBox onSearch={handleSearch} />)

const input = screen.getByRole('textbox', { name: /search/i })
await user.type(input, 'laptop{Enter}')

expect(handleSearch).toHaveBeenCalledWith('laptop')
```

#### Test: Shows clear button when input has value
```typescript
render(<SearchBox defaultValue="test" />)

expect(screen.getByRole('button', { name: /clear/i })).toBeInTheDocument()

await user.click(screen.getByRole('button', { name: /clear/i }))

expect(screen.getByRole('textbox')).toHaveValue('')
```

### Component: DocumentCard

**File:** `src/components/search/DocumentCard.test.tsx` _(TODO: unit test file not yet implemented)_

#### Test: Renders document summary in collapsed state
```typescript
const mockDocument = {
  objectID: '123',
  title: 'MacBook Pro',
  description: 'Powerful laptop',
}

render(<DocumentCard document={mockDocument} />)

expect(screen.getByText('MacBook Pro')).toBeInTheDocument()
expect(screen.getByText('Powerful laptop')).toBeInTheDocument()
expect(screen.queryByText('{}')).not.toBeInTheDocument() // JSON not visible
```

#### Test: Expands to show JSON when button clicked
```typescript
render(<DocumentCard document={mockDocument} />)

const expandButton = screen.getByRole('button', { name: /expand|view json/i })
await user.click(expandButton)

expect(screen.getByText(/"objectID"/)).toBeInTheDocument() // JSON key visible
```

### Component: FacetsPanel

**File:** `src/components/search/FacetsPanel.test.tsx` _(TODO: unit test file not yet implemented)_

#### Test: Renders facet values with counts
```typescript
const mockFacets = {
  brand: {
    'Apple': 3,
    'Dell': 1,
    'Samsung': 2,
  },
}

render(<FacetsPanel facets={mockFacets} />)

expect(screen.getByText('Apple')).toBeInTheDocument()
expect(screen.getByText('3')).toBeInTheDocument()
```

#### Test: Calls onFilterChange when facet value clicked
```typescript
const handleFilter = vi.fn()

render(<FacetsPanel facets={mockFacets} onFilterChange={handleFilter} />)

await user.click(screen.getByText('Apple'))

expect(handleFilter).toHaveBeenCalledWith({
  facet: 'brand',
  value: 'Apple',
})
```

#### Test: Shows message when no facets configured
```typescript
render(<FacetsPanel facets={{}} />)

expect(screen.getByText(/no facets configured/i)).toBeInTheDocument()
```

#### Test: Shows facets even when results are empty (bug fix)
```typescript
// Regression test for B-SEARCH-002-BUG
const mockFacets = {
  brand: { 'Apple': 0 }, // 0 count but facet still exists
}

render(<FacetsPanel facets={mockFacets} resultsCount={0} />)

// Should show facets, not "No facets configured"
expect(screen.getByText('brand')).toBeInTheDocument()
expect(screen.getByText('Apple')).toBeInTheDocument()
```

### Hook: useSearch

**File:** `src/hooks/useSearch.test.ts` _(TODO: unit test file not yet implemented)_

#### Test: Fetches search results on query change
```typescript
const { result } = renderHook(() => useSearch({ index: 'products', query: 'laptop' }), {
  wrapper: createQueryWrapper(),
})

await waitFor(() => {
  expect(result.current.isLoading).toBe(false)
})

expect(result.current.data.hits).toHaveLength(5)
```

---

## Acceptance Criteria Summary

**Unit Tests:**
- ✅ SearchBox component (3 tests)
- ✅ DocumentCard component (3 tests)
- ✅ FacetsPanel component (5 tests including regression)
- ✅ ResultsPanel component (2 tests)
- ✅ useSearch hook (3 tests)

**E2E Smoke Tests:**
- ✅ Search with Enter key (1 test)

**E2E Full Tests:**
- ✅ Search with typo tolerance (1 test)
- ✅ Filter by single facet (1 test)
- ✅ Filter producing 0 results (1 test - regression)
- ✅ Clear facet filters (1 test)
- ✅ Paginate results (1 test)
- ✅ Expand document JSON (1 test)

**Total:** 16 unit + 1 smoke + 6 full E2E = 23 tests for search & browse

---

## TEST: Facets remain visible when filter produces 0 results (B-SEARCH-005)

**User Story:** B-SEARCH-005
**Type:** E2E-UI (route mocking)
**File:** `tests/e2e-ui/full/search-facets-regression.spec.ts`

### Setup
- Mock search endpoint: first call returns results with facets, second call (with facetFilters) returns 0 hits
- Navigate to `/index/test-index`

### Execute
1. Wait for facets panel to load with facet values
2. Click a facet value to apply filter
3. Mock returns 0 results for filtered query

### Verify UI
- Facets panel is still visible (data-testid="facets-panel")
- Facets panel does NOT show "No facets configured"
- Active filter is shown and can be cleared
- Clear button is visible

### Expected Values
- Facets panel: visible
- "No facets configured": NOT visible

---

## TEST: Facet counts reflect current search (B-SEARCH-006)

**User Story:** B-SEARCH-006
**Type:** E2E-UI (route mocking)
**File:** `tests/e2e-ui/full/search-facets-regression.spec.ts`

### Setup
- Mock search endpoint: return different facet counts based on query

### Execute
1. Navigate to `/index/test-index`
2. Verify initial facet counts
3. Type a search query
4. Wait for search results to update

### Verify UI
- Facet counts update to reflect filtered results
- Counts match the search response facet data

---

## Known Issues

- **B-SEARCH-002-BUG:** FacetsPanel made redundant query — FIXED: uses separate unfiltered query for facet list
- **Performance:** Analytics fetches all data on mount - needs lazy loading

---

## Notes

- Facets panel bug has regression test to prevent recurrence
- All tests use fixtures with expected values
- E2E-UI tests use route mocking (no real backend required)
- Unit tests mock API responses
