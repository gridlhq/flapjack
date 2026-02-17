# Test Specification: Index Management (Tier 2)

**Feature:** Index CRUD operations
**BDD Spec:** B-IDX-001, B-IDX-002, B-IDX-003
**Priority:** P1
**Last Updated:** 2026-02-13

---

## Test Fixtures

**Location:** `tests/fixtures/test-data.ts`

```typescript
export const validIndexName = 'test-index-1234'
export const invalidIndexNames = {
  uppercase: 'TestIndex',
  underscore: 'test_index',
  special: 'test@index',
  empty: '',
}

export const mockIndexResponse = {
  name: 'test-index-1234',
  createdAt: '2026-02-13T10:30:00Z',
  updatedAt: '2026-02-13T10:30:00Z',
  nbDocuments: 0,
  settings: { /* ... */ },
}
```

---

## TEST: View index overview

**User Story:** B-IDX-001
**Type:** E2E Integration
**File:** `tests/e2e-ui/smoke/overview.spec.ts`

### Setup
- Backend server running on port 7700
- At least 1 index exists in database

### Execute
1. Navigate to `/` (overview page)
2. Wait for page load

### Verify UI
- `[data-testid="stat-card"]` - Expect count = 4
- `[data-testid="stat-card"]` first card shows "Indexes" label
- `[data-testid="index-list"]` is visible
- `[role="button"][name=/create.*index/i]` is visible

### Verify API
- GET `/indexes` returns 200
- Response is array of index objects

### Expected Values
- Stats cards: 4 cards (Indexes, Documents, Storage, Status/Health)
- Index list: >= 1 index displayed
- Pagination: visible if > 10 indexes

### Cleanup
- None (read-only test)

---

## TEST: Create index with valid name

**User Story:** B-IDX-002
**Type:** E2E Full
**File:** `tests/e2e-ui/full/index-management.spec.ts`

### Fixtures
- `validIndexName` = 'test-index-1234'

### Setup
- Backend server running
- Index with `validIndexName` does NOT exist

### Execute
1. Navigate to `/`
2. Click `[role="button"][name=/create.*index/i]`
3. Wait for dialog to open
4. Fill `[label="Index Name"]` with `validIndexName`
5. Click `[role="button"][name=/submit|create/i]`
6. Wait for success toast

### Verify UI
- Toast message: "Index created successfully"
- Dialog closes automatically
- `[data-testid="index-list"]` contains `validIndexName`
- New index card shows 0 documents

### Verify API
- POST `/indexes` with body `{ name: validIndexName }` returns 201
- GET `/indexes` returns array including `validIndexName`

### Expected Values
- Index name: `validIndexName`
- Document count: 0
- Created timestamp: within last 10 seconds

### Cleanup
```typescript
await page.request.delete(`http://localhost:7700/indexes/${validIndexName}`)
```

---

## TEST: Create index with invalid name (uppercase)

**User Story:** B-IDX-002 (edge case)
**Type:** Unit + E2E
**File:** `src/lib/validators.test.ts` + `tests/e2e-ui/full/index-management.spec.ts`

### Fixtures
- `invalidIndexNames.uppercase` = 'TestIndex'

### Setup
- Backend server running

### Execute
1. Navigate to `/`
2. Click create index button
3. Fill input with `invalidIndexNames.uppercase`
4. Attempt to submit

### Verify UI
- Error message: "Index name must be lowercase"
- Submit button disabled OR form shows validation error
- Dialog remains open
- Index list unchanged

### Verify API
- POST request NOT sent (client-side validation blocks)

### Expected Values
- Error message text: exact match "Index name must be lowercase"

### Cleanup
- None (no index created)

---

## TEST: Delete index with confirmation

**User Story:** B-IDX-003
**Type:** E2E Full
**File:** `tests/e2e-ui/full/index-management.spec.ts`

### Fixtures
- Create temporary index: `test-delete-index`

### Setup
```typescript
// Create index to delete
await page.request.post('http://localhost:7700/indexes/test-delete-index')
```

### Execute
1. Navigate to `/`
2. Find index card for `test-delete-index`
3. Click `[role="button"][name=/delete.*test-delete-index/i]`
4. Wait for confirmation dialog
5. Verify dialog shows index name: "test-delete-index"
6. Click `[role="button"][name=/confirm|delete/i]`
7. Wait for success toast

### Verify UI
- Confirmation dialog displays before deletion
- Dialog shows correct index name
- Success toast: "Index deleted successfully"
- Dialog closes after confirmation
- Index card removed from list immediately
- Index count stat decremented by 1

### Verify API
- DELETE `/indexes/test-delete-index` returns 200
- GET `/indexes` does NOT include `test-delete-index`

### Expected Values
- Toast message: "Index deleted successfully"
- Index list: does not contain `test-delete-index`

### Cleanup
```typescript
// Cleanup if test fails
try {
  await page.request.delete('http://localhost:7700/indexes/test-delete-index')
} catch {}
```

---

## TEST: Cancel delete operation

**User Story:** B-IDX-003 (edge case)
**Type:** E2E Full
**File:** `tests/e2e-ui/full/index-management.spec.ts`

### Setup
- Index `test-cancel-delete` exists

### Execute
1. Navigate to `/`
2. Click delete button on `test-cancel-delete`
3. Wait for confirmation dialog
4. Click `[role="button"][name=/cancel/i]`

### Verify UI
- Dialog closes
- Index still present in list
- No toast message shown

### Verify API
- DELETE request NOT sent

### Expected Values
- Index list: still contains `test-cancel-delete`

### Cleanup
```typescript
await page.request.delete('http://localhost:7700/indexes/test-cancel-delete')
```

---

## Unit Test Specifications

### Component: IndexCard

**File:** `src/components/IndexCard.test.tsx` _(TODO: unit test file not yet implemented)_

#### Test: Renders index name and stats
```typescript
const mockIndex = {
  name: 'my-index',
  nbDocuments: 1234,
  updatedAt: '2026-02-13T10:30:00Z',
}

render(<IndexCard index={mockIndex} />)

expect(screen.getByText('my-index')).toBeInTheDocument()
expect(screen.getByText('1,234 documents')).toBeInTheDocument()
expect(screen.getByText(/Feb 13, 2026/)).toBeInTheDocument()
```

#### Test: Calls onDelete when delete button clicked
```typescript
const handleDelete = vi.fn()

render(<IndexCard index={mockIndex} onDelete={handleDelete} />)

const deleteButton = screen.getByRole('button', { name: /delete/i })
await user.click(deleteButton)

expect(handleDelete).toHaveBeenCalledWith('my-index')
```

### Component: CreateIndexDialog

**File:** `src/components/CreateIndexDialog.test.tsx` _(TODO: unit test file not yet implemented)_

#### Test: Validates index name on blur
```typescript
render(<CreateIndexDialog open={true} />)

const input = screen.getByLabelText('Index Name')
await user.type(input, 'TestIndex')
await user.tab() // Trigger blur

expect(screen.getByText('Index name must be lowercase')).toBeInTheDocument()
```

#### Test: Submits valid form
```typescript
const handleCreate = vi.fn().mockResolvedValue({ name: 'test-index' })

render(<CreateIndexDialog open={true} onSubmit={handleCreate} />)

await user.type(screen.getByLabelText('Index Name'), 'test-index')
await user.click(screen.getByRole('button', { name: /create/i }))

await waitFor(() => {
  expect(handleCreate).toHaveBeenCalledWith({ name: 'test-index' })
})
```

### Utility: validateIndexName

**File:** `src/lib/validators.test.ts` _(TODO: unit test file not yet implemented)_

#### Test: Accepts valid names
```typescript
expect(validateIndexName('my-index')).toBe(true)
expect(validateIndexName('test123')).toBe(true)
expect(validateIndexName('a')).toBe(true)
```

#### Test: Rejects invalid names
```typescript
expect(validateIndexName('TestIndex')).toBe(false) // uppercase
expect(validateIndexName('test_index')).toBe(false) // underscore
expect(validateIndexName('test@index')).toBe(false) // special char
expect(validateIndexName('')).toBe(false) // empty
```

---

## Acceptance Criteria Summary

**Unit Tests:**
- ✅ IndexCard component (3 tests)
- ✅ CreateIndexDialog component (3 tests)
- ✅ DeleteConfirmDialog component (2 tests)
- ✅ validateIndexName utility (2 tests)

**E2E Smoke Tests:**
- ✅ View overview page (1 test)

**E2E Full Tests:**
- ✅ Create index with valid name (1 test)
- ✅ Create index with invalid name (1 test)
- ✅ Delete index with confirmation (1 test)
- ✅ Cancel delete operation (1 test)

**Total:** 10 unit + 1 smoke + 4 full E2E = 15 tests for index management

---

## Notes

- All tests use `data-testid` for complex selectors
- All tests use role-based queries where possible (accessibility)
- Fixtures include expected values for deterministic assertions
- Each E2E test includes cleanup in try/finally
- Unit tests mock API calls
- E2E tests use real backend API
