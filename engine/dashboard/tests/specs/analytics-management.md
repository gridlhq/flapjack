# Analytics Management — Tier 2: Test Specifications

**Maps to:** B-ANA-008, B-ANA-009, B-ANA-010
**Test Type:** E2E-UI (route mocking, no real backend)
**Prerequisites:** Dashboard dev server running

---

## Test Data Setup

**Mock Fixtures:**
```typescript
analyticsSearchCount: { count: 500, dates: [...] }
analyticsUsersCount: { count: 50 }
analyticsNoResultRate: { rate: 0.05, dates: [...] }
analyticsCleared: { count: 0, dates: [] }
```

---

## B-ANA-008: Clear Analytics with Confirmation Dialog

### TEST: Clear button opens ConfirmDialog (not browser confirm)
**Execute:**
1. Mock all analytics endpoints with data
2. Navigate to `/index/products/analytics`
3. Wait for analytics data to load
4. Click "Clear Analytics" button

**Verify UI:**
- A styled dialog overlay appears (not browser native confirm)
- Dialog contains the index name "products"
- Dialog has a "Cancel" button
- Dialog has a destructive "Clear" button
- No browser confirm() was triggered

### TEST: Confirming clear removes cached data immediately
**Execute:**
1. Mock all analytics endpoints (return data initially, return 0 after clear)
2. Navigate to `/index/products/analytics`
3. Wait for KPI cards to show data (e.g., "500" total searches)
4. Click "Clear Analytics"
5. Click "Clear" in the confirmation dialog
6. Wait for clear mutation to complete

**Verify UI:**
- KPI cards immediately show 0 (not stale cached data)
- Success message "Analytics cleared" is visible

**Verify API:**
- DELETE /2/analytics/clear was called with index: "products"

### TEST: Canceling clear keeps data intact
**Execute:**
1. Mock analytics endpoints with data
2. Navigate to `/index/products/analytics`
3. Click "Clear Analytics"
4. Click "Cancel" in dialog

**Verify UI:**
- Dialog closes
- KPI values remain unchanged (still show original data)

**Verify API:**
- No DELETE request was sent

---

## B-ANA-009: Update (Flush) Analytics

### TEST: Update button flushes and refreshes data
**Execute:**
1. Mock analytics endpoints
2. Mock POST /2/analytics/flush → 200
3. Navigate to `/index/products/analytics`
4. Click "Update" button

**Verify UI:**
- Button shows loading state ("Updating..." text, spinning icon)
- After completion, button returns to normal state

**Verify API:**
- POST /2/analytics/flush was called

---

## B-ANA-010: Analytics BETA Label

### TEST: BETA badge is visible with orange/red styling
**Execute:**
1. Mock analytics endpoints
2. Navigate to `/index/products/analytics`

**Verify UI:**
- Element containing text "BETA" is visible near the heading
- Badge has orange or red background color (check class or computed style)
