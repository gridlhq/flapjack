# Dashboard Testing Guide

**Project:** Flapjack Dashboard (React + TypeScript)
**Last Updated:** 2026-02-13

**Read `AI_TESTING_METHODOLOGY.md` first** - this document is dashboard-specific guidance.

---

## Quick Reference

```bash
# Unit tests (Vitest + RTL)
npm run test:unit              # Run all unit tests (watch mode)
npm run test:unit:run          # Run once (CI mode)
npm run test:unit:coverage     # With coverage report

# E2E tests (Playwright)
npm test                       # All E2E tests
npm run test:smoke             # Smoke tests only (~2 min)
npm run test:e2e               # Full E2E suite (~10-15 min)
npm run test:ui                # Playwright UI mode (debugging)
npm run test:debug             # Debug mode (step through)

# Integration tests
npm run test:integration       # Playwright integration tests
```

---

## Test Distribution (Dashboard-Specific)

Following the 65/25/10 rule adapted for React:

### **65% Unit Tests** - Vitest + React Testing Library
**What:** Test components, hooks, utilities in isolation
**Where:** `src/**/*.test.{ts,tsx}`
**Speed:** < 5s for full suite (watch mode: < 1s incremental)

**Test these:**
- ✅ Component rendering & props
- ✅ User interactions (clicks, typing, form submission)
- ✅ Custom hooks (useState, useEffect, React Query, Zustand)
- ✅ Utility functions (formatters, validators, API clients)
- ✅ Conditional rendering logic
- ✅ Error boundaries

**Don't test:**
- ❌ Implementation details (state variable names, internal functions)
- ❌ Third-party libraries (React Router, React Query internals)
- ❌ Styles (focus on behavior, not CSS)

### **25% Integration Tests** - Playwright with Real Backend
**What:** Multi-component flows with real API calls
**Where:** `tests/integration/*.spec.ts`
**Speed:** ~5-8 min for full suite

**Test these:**
- ✅ Create index → see it in list
- ✅ Search → filter facets → verify results
- ✅ Edit settings → save → reload → verify persisted
- ✅ Create API key → copy → verify in list
- ✅ Navigation between pages with state

**Don't test:**
- ❌ Individual component behavior (use unit tests)
- ❌ Full user journeys (use E2E smoke/full)

### **10% E2E UI Tests** - Playwright Full User Journeys
**What:** Complete workflows from entry to completion
**Where:** `tests/e2e/smoke/*.spec.ts` and `tests/e2e/full/*.spec.ts`
**Speed:** Smoke ~2 min, Full ~10-15 min

**Smoke tests (5-7 critical paths):**
- ✅ Load overview → verify stats cards
- ✅ Navigate to search → enter query → see results
- ✅ Navigate to settings → modify → save
- ✅ Navigate to API keys → verify list loads
- ✅ Navigate to analytics → verify charts load

**Full E2E tests (30-50 comprehensive):**
- ✅ All behaviors from BDD_SPECIFICATIONS.md
- ✅ Edge cases (empty states, errors, validation)
- ✅ Multi-step flows (create → search → filter → paginate)

---

## Writing Unit Tests (Vitest + RTL)

### Component Test Pattern

```typescript
// src/components/MyComponent.test.tsx
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { MyComponent } from './MyComponent'

describe('MyComponent', () => {
  it('renders with props', () => {
    render(<MyComponent title="Hello" count={5} />)

    expect(screen.getByText('Hello')).toBeInTheDocument()
    expect(screen.getByText('5')).toBeInTheDocument()
  })

  it('calls onClick when button is clicked', async () => {
    const user = userEvent.setup()
    const handleClick = vi.fn()

    render(<MyComponent onClick={handleClick} />)

    const button = screen.getByRole('button', { name: /submit/i })
    await user.click(button)

    expect(handleClick).toHaveBeenCalledOnce()
  })

  it('shows error state when error prop is provided', () => {
    render(<MyComponent error="Something went wrong" />)

    expect(screen.getByText('Something went wrong')).toBeInTheDocument()
    expect(screen.getByRole('alert')).toBeInTheDocument()
  })
})
```

### Custom Hook Test Pattern

```typescript
// src/hooks/useMyHook.test.ts
import { renderHook, waitFor } from '@testing-library/react'
import { describe, it, expect } from 'vitest'
import { useMyHook } from './useMyHook'

describe('useMyHook', () => {
  it('returns initial state', () => {
    const { result } = renderHook(() => useMyHook())

    expect(result.current.count).toBe(0)
    expect(result.current.isLoading).toBe(false)
  })

  it('increments count when increment is called', () => {
    const { result } = renderHook(() => useMyHook())

    act(() => {
      result.current.increment()
    })

    expect(result.current.count).toBe(1)
  })
})
```

### Utility Function Test Pattern

```typescript
// src/lib/utils.test.ts
import { describe, it, expect } from 'vitest'
import { formatDate, validateIndexName } from './utils'

describe('formatDate', () => {
  it('formats ISO date to readable string', () => {
    const result = formatDate('2026-02-13T10:30:00Z')
    expect(result).toBe('Feb 13, 2026')
  })

  it('handles invalid dates', () => {
    const result = formatDate('invalid')
    expect(result).toBe('Invalid date')
  })
})

describe('validateIndexName', () => {
  it('accepts valid names', () => {
    expect(validateIndexName('my-index')).toBe(true)
    expect(validateIndexName('test123')).toBe(true)
  })

  it('rejects invalid names', () => {
    expect(validateIndexName('MyIndex')).toBe(false) // uppercase
    expect(validateIndexName('my_index')).toBe(false) // underscore
    expect(validateIndexName('')).toBe(false) // empty
  })
})
```

---

## Writing E2E Tests (Playwright)

### Smoke Test Pattern

```typescript
// tests/e2e/smoke/overview.spec.ts
import { test, expect } from '@playwright/test'

test.describe('Overview Page (Smoke)', () => {
  test('loads and displays stats cards', async ({ page }) => {
    await page.goto('/')

    // Verify critical elements load
    const statCards = page.getByTestId('stat-card')
    await expect(statCards).toHaveCount(4)

    // Verify navigation works
    await page.getByRole('link', { name: /search/i }).click()
    await expect(page).toHaveURL(/\/search/)
  })
})
```

### Full E2E Test Pattern

```typescript
// tests/e2e/full/index-management.spec.ts
import { test, expect } from '@playwright/test'

test.describe('Index Management', () => {
  test('creates new index', async ({ page }) => {
    await page.goto('/')

    // Click create button
    await page.getByRole('button', { name: /create.*index/i }).click()

    // Fill form
    await page.getByLabel('Index Name').fill('test-index')
    await page.getByRole('button', { name: /submit/i }).click()

    // Verify success
    await expect(page.getByText('Index created successfully')).toBeVisible()
    await expect(page.getByText('test-index')).toBeVisible()

    // Cleanup
    await page.getByRole('button', { name: /delete.*test-index/i }).click()
    await page.getByRole('button', { name: /confirm/i }).click()
  })
})
```

---

## React Query Testing

**Use `QueryClientProvider` wrapper:**

```typescript
// tests/utils/query-wrapper.tsx
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ReactNode } from 'react'

export function createQueryWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })

  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  )
}
```

**Use in tests:**

```typescript
import { render } from '@testing-library/react'
import { createQueryWrapper } from '@/tests/utils/query-wrapper'
import { MyComponent } from './MyComponent'

it('renders with query data', async () => {
  render(<MyComponent />, { wrapper: createQueryWrapper() })

  await waitFor(() => {
    expect(screen.getByText('Loaded data')).toBeInTheDocument()
  })
})
```

---

## Zustand Store Testing

**Test stores directly:**

```typescript
// src/stores/useIndexStore.test.ts
import { describe, it, expect, beforeEach } from 'vitest'
import { useIndexStore } from './useIndexStore'

describe('useIndexStore', () => {
  beforeEach(() => {
    // Reset store before each test
    useIndexStore.setState({ selectedIndex: null })
  })

  it('sets selected index', () => {
    const { setSelectedIndex } = useIndexStore.getState()

    setSelectedIndex('my-index')

    const { selectedIndex } = useIndexStore.getState()
    expect(selectedIndex).toBe('my-index')
  })
})
```

---

## Best Practices (2026)

### ✅ DO

1. **Use role-based queries** (getByRole) for better accessibility
   ```typescript
   screen.getByRole('button', { name: /submit/i })
   ```

2. **Test behavior, not implementation**
   ```typescript
   // ✅ Good - tests user-visible behavior
   expect(screen.getByText('5 results')).toBeInTheDocument()

   // ❌ Bad - tests implementation
   expect(component.state.count).toBe(5)
   ```

3. **Use user-event for interactions**
   ```typescript
   const user = userEvent.setup()
   await user.click(button)
   await user.type(input, 'test')
   ```

4. **Add testIDs for complex selectors**
   ```tsx
   <div data-testid="facets-panel">...</div>
   ```

5. **Mock only external dependencies**
   - Mock fetch/axios
   - Mock external APIs
   - Don't mock React Query or Zustand

6. **Use fixtures with expected values**
   ```typescript
   const fixture = {
     metadata: { expected_count: 10 },
     documents: [...]
   }
   expect(result).toBe(fixture.metadata.expected_count)
   ```

### ❌ DON'T

1. **Don't test implementation details**
   - State variable names
   - Function names
   - Component structure

2. **Don't use shallow rendering**
   - Use full render with RTL

3. **Don't snapshot test everything**
   - Use snapshots sparingly (error messages, complex UI)

4. **Don't test third-party libraries**
   - Trust React Router, React Query, etc.

5. **Don't make tests dependent on each other**
   - Each test should be independent
   - Use beforeEach for setup

---

## Coverage Targets

### Unit Tests
- **Lines:** 85%+
- **Functions:** 85%+
- **Branches:** 80%+
- **Statements:** 85%+

### E2E Tests
- **Smoke:** 5-7 critical paths
- **Full:** 30-50 comprehensive tests
- **Coverage:** 100% of user-facing features

---

## When to Run Tests

| Test Type | Trigger | Speed | Purpose |
|-----------|---------|-------|---------|
| Unit | On file save (watch mode) | < 1s | Instant feedback during development |
| Unit (full) | Before commit | < 5s | Ensure all units pass |
| Smoke E2E | On commit (CI) | ~2 min | Critical paths protected |
| Integration | Before PR | ~5-8 min | Multi-component flows validated |
| Full E2E | Before release | ~10-15 min | Comprehensive validation |

---

## CI/CD Integration

**GitHub Actions workflow:**

```yaml
name: Tests

on: [push, pull_request]

jobs:
  unit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: npm ci
      - run: npm run test:unit:run
      - run: npm run test:unit:coverage

  e2e-smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: npm ci
      - run: npx playwright install
      - run: npm run server &
      - run: npm run test:smoke

  e2e-full:
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v3
      - run: npm ci
      - run: npx playwright install
      - run: npm run server &
      - run: npm run test:e2e
```

---

## Debugging Tests

### Unit Tests (Vitest)

```bash
# Run specific test file
npm run test:unit -- MyComponent.test.tsx

# Run tests matching pattern
npm run test:unit -- --grep "renders correctly"

# Debug in VS Code
# Add breakpoint, then F5 with "Debug Vitest" config
```

### E2E Tests (Playwright)

```bash
# UI mode (best for debugging)
npm run test:ui

# Headed mode (see browser)
npm run test:headed

# Debug mode (step through)
npm run test:debug

# Run specific test
npm test -- overview.spec.ts
```

---

## Common Issues

### Issue: React Query tests timeout

**Solution:** Use `waitFor` and set proper retry: false

```typescript
const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false },
  },
})
```

### Issue: Zustand tests have stale state

**Solution:** Reset store in beforeEach

```typescript
beforeEach(() => {
  useMyStore.setState({ count: 0 })
})
```

### Issue: Playwright tests fail on CI but pass locally

**Solution:** Start backend server before tests

```bash
npm run server &
sleep 5  # Wait for server to start
npm test
```

---

## Resources

**Testing Libraries:**
- [Vitest](https://vitest.dev/guide/) (10-20x faster than Jest)
- [React Testing Library](https://testing-library.com/docs/react-testing-library/intro/)
- [Playwright](https://playwright.dev/)
- [Testing Library Queries](https://testing-library.com/docs/queries/about/)

**Best Practices (2026):**
- [How to Unit Test React Components with Vitest and React Testing Library](https://oneuptime.com/blog/post/2026-01-15-unit-test-react-vitest-testing-library/view)
- [Vitest with React Testing Library: A Modern Approach](https://blog.incubyte.co/blog/vitest-react-testing-library-guide/)
- [15 Best Practices for Playwright testing in 2026](https://www.browserstack.com/guide/playwright-best-practices)

---

**Remember:** Tests are the specification. They document what the code should do. Write them first, make them pass, then refactor.
