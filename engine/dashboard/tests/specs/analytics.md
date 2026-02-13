# Analytics Feature — Tier 2: Test Specifications

**Maps to:** B-ANA-001 through B-ANA-007
**Test Type:** UI E2E (real backend + database, NO mocks)
**Prerequisites:** Flapjack server running on localhost:7700

---

## Test Data Setup

**Before all tests:**
1. Create test index `e2e-analytics-test`
2. Seed 100 documents
3. Execute 500 searches with known patterns
4. Wait for analytics flush (or force flush)

**Expected analytics data:**
- Total searches: 500
- Unique users: 50
- No-result queries: 25 (5% rate)
- Devices: Desktop 60%, Mobile 30%, Tablet 10%
- Countries: US 45%, GB 20%, DE 15%, CA 10%, FR 10%
- Top query: "laptop" (50 searches, 12 results)

**After all tests:**
- Delete test index

---

## B-ANA-001: View Analytics Overview

### TEST: Overview tab loads with KPI cards
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Wait for [data-testid="kpi-cards"] visible

**Verify UI:**
- [data-testid="analytics-heading"] shows "Analytics"
- [data-testid="kpi-total-searches"] shows "500"
- [data-testid="kpi-unique-users"] shows "50"
- [data-testid="kpi-no-result-rate"] shows "5.0%"

**Verify API:**
- GET /2/searches/count returned { count: 500, dates: [...] }
- GET /2/users/count returned { count: 50 }

**Cleanup:** None

### TEST: Search volume chart renders with data
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Wait for [data-testid="search-volume-chart"] visible

**Verify UI:**
- Chart contains SVG element
- Chart has area path (recharts-area-area class)
- Chart shows 7 data points (7 days)

**Verify API:**
- dates array has 7 entries

**Cleanup:** None

### TEST: Switching to 30d updates all data
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Wait for [data-testid="kpi-cards"] visible
3. Get value from [data-testid="kpi-total-searches"]
4. Click [data-testid="range-30d"]
5. Wait for new API call to complete

**Verify UI:**
- [data-testid="range-30d"] has class "bg-primary"
- [data-testid="range-7d"] does NOT have class "bg-primary"
- KPI values update (30d >= 7d)

**Verify API:**
- New GET /2/searches/count with startDate 30 days ago

**Cleanup:** None

---

## B-ANA-002: Analyze Search Queries

### TEST: Searches tab shows query table
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click [data-testid="tab-searches"]
3. Wait for [data-testid="top-searches-table"] visible

**Verify UI:**
- Table shows rows (tbody tr count > 0)
- First row shows "laptop" (top query)
- First row shows count "50"
- First row shows avg hits "12"

**Verify API:**
- GET /2/searches returned searches array
- searches[0].search === "laptop"
- searches[0].count === 50

**Cleanup:** None

### TEST: Text filter narrows results client-side
**Prerequisites:** On searches tab
**Execute:**
1. Get initial row count
2. Type "laptop" in [data-testid="searches-filter-input"]
3. Wait for filter to apply

**Verify UI:**
- Row count decreases
- All visible rows contain "laptop"
- Query count label updates (e.g., "5 queries")

**Verify API:**
- No new API call (client-side filter)

**Cleanup:** Clear filter input

### TEST: Country filter triggers server-side filter
**Prerequisites:** On searches tab
**Execute:**
1. Get top query count (unfiltered)
2. Select "US" from [data-testid="searches-country-filter"]
3. Wait for new API call

**Verify UI:**
- Top query count changes (filtered < unfiltered)

**Verify API:**
- GET /2/searches with country=US parameter

**Cleanup:** Reset to "All Countries"

### TEST: Device filter triggers server-side filter
**Prerequisites:** On searches tab
**Execute:**
1. Select "desktop" from [data-testid="searches-device-filter"]
2. Wait for new API call

**Verify UI:**
- Query counts update

**Verify API:**
- GET /2/searches with tags=platform:desktop parameter

**Cleanup:** Reset to "All Devices"

---

## B-ANA-003: Identify No-Result Searches

### TEST: No Results tab shows rate banner and table
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click [data-testid="tab-no-results"]
3. Wait for [data-testid="no-result-rate-banner"] visible

**Verify UI:**
- Banner shows "5.0%" rate
- Banner contains "of searches return no results"
- [data-testid="no-results-table"] visible
- Table has 25 rows (all no-result queries)
- All rows have nbHits === 0

**Verify API:**
- GET /2/searches/noResultRate returned { rate: 0.05 }
- GET /2/searches/noResults returned 25 searches

**Cleanup:** None

---

## B-ANA-004: Understand Device Distribution

### TEST: Devices tab shows platform cards
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click [data-testid="tab-devices"]
3. Wait for [data-testid="device-desktop"] visible

**Verify UI:**
- [data-testid="device-desktop"] shows "300" (60%)
- [data-testid="device-mobile"] shows "150" (30%)
- [data-testid="device-tablet"] shows "50" (10%)
- Percentages displayed: "60.0%", "30.0%", "10.0%"

**Verify API:**
- GET /2/devices returned platforms array
- platforms[0] === { platform: "desktop", count: 300 }

**Cleanup:** None

### TEST: Device chart renders with stacked areas
**Prerequisites:** On devices tab
**Execute:**
1. Wait for chart heading "Searches by Device Over Time" visible

**Verify UI:**
- Chart contains SVG
- SVG has multiple area paths (one per platform)

**Verify API:**
- dates array has entries with platform breakdowns

**Cleanup:** None

---

## B-ANA-005: Analyze Geographic Distribution

### TEST: Geography tab shows country table
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click [data-testid="tab-geography"]
3. Wait for country table visible

**Verify UI:**
- [data-testid="geo-countries-count"] shows "5" (5 countries)
- Table shows "United States" (top country)
- US row shows "225" (45%)
- Percentages sum to ~100%

**Verify API:**
- GET /2/geo returned countries array
- countries[0] === { country: "US", count: 225 }

**Cleanup:** None

### TEST: Clicking country drills down to regions
**Prerequisites:** On geography tab
**Execute:**
1. Click row with "United States"
2. Wait for drill-down view

**Verify UI:**
- Button "All Countries" visible
- Heading "Top Searches from United States" visible
- States table shows regions (California, New York, etc.)

**Verify API:**
- GET /2/geo/US returned top searches for US
- GET /2/geo/US/regions returned regions array

**Cleanup:** None

### TEST: Back button returns to country list
**Prerequisites:** In drill-down view
**Execute:**
1. Click button "All Countries"

**Verify UI:**
- Country table visible again
- Drill-down view hidden

**Verify API:**
- No new API call (client-side navigation)

**Cleanup:** None

---

## B-ANA-006: Analyze Filter Usage

### TEST: Filters tab shows filter attributes
**Prerequisites:** Analytics data seeded with faceted searches
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click [data-testid="tab-filters"]
3. Wait for [data-testid="filters-table"] visible

**Verify UI:**
- Table shows filter attributes (brand:Apple, category:Laptops, etc.)
- Each row shows count

**Verify API:**
- GET /2/filters returned filters array

**Cleanup:** None

### TEST: Expanding filter shows values
**Prerequisites:** On filters tab
**Execute:**
1. Click first filter row
2. Wait for child values to load

**Verify UI:**
- Expanded row shows filter values
- Values have counts

**Verify API:**
- GET /2/filters/{attribute} returned values array

**Cleanup:** None

---

## B-ANA-007: Manage Analytics Data

### TEST: Update button flushes analytics
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click button "Update"
3. Wait for response

**Verify UI:**
- No error shown

**Verify API:**
- POST /2/analytics/flush returned 200

**Cleanup:** None

### TEST: Clear button deletes analytics after confirmation
**Prerequisites:** Analytics data seeded
**Execute:**
1. Navigate to `/index/e2e-analytics-test/analytics`
2. Click button "Clear Analytics"
3. Accept confirm dialog

**Verify UI:**
- Dialog message contains index name "e2e-analytics-test"
- After confirm, KPIs show 0

**Verify API:**
- DELETE /2/analytics/clear returned 200
- GET /2/searches/count now returns { count: 0 }

**Cleanup:** Re-seed analytics for other tests

### TEST: Clear button does not delete when canceled
**Prerequisites:** Analytics data seeded
**Execute:**
1. Click button "Clear Analytics"
2. Dismiss confirm dialog

**Verify UI:**
- KPIs still show data (not 0)

**Verify API:**
- No DELETE request sent

**Cleanup:** None

---

## Empty States

### TEST: Empty state shown when no analytics data
**Prerequisites:** Index exists but has no analytics
**Execute:**
1. Create index `e2e-empty-analytics`
2. Navigate to `/index/e2e-empty-analytics/analytics`

**Verify UI:**
- [data-testid="empty-state"] visible
- Heading "No search data yet" visible
- KPIs show 0

**Verify API:**
- GET /2/searches/count returned { count: 0, dates: [] }

**Cleanup:** Delete index

---

## Error States

### TEST: Error state shown when API fails
**Prerequisites:** Flapjack server stopped
**Execute:**
1. Navigate to `/index/test-index/analytics`

**Verify UI:**
- [data-testid="error-state"] visible
- Error message shown

**Verify API:**
- GET /2/searches/count failed with network error

**Cleanup:** Restart server
