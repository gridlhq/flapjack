# Analytics Page

Test index: `e2e-products` (7 days of analytics data pre-seeded)

## analytics-1: Overview tab loads with data (SMOKE)
1. Go to /index/e2e-products/analytics
2. See the "Analytics" heading with BETA badge
3. See the date range selector (7d, 30d, 90d) with 7d selected
4. See KPI cards: Total Searches, Unique Users, No-Result Rate
5. See at least one KPI card showing a non-zero value
6. See the "Update" and "Clear Analytics" buttons in the header

## analytics-2: Search volume chart visible
1. Go to /index/e2e-products/analytics
2. See the "Search Volume" chart card
3. See the chart rendered with data points (not empty state)
4. See the chart axes (date on X, count on Y)

## analytics-3: Top searches table
1. Go to /index/e2e-products/analytics
2. See the "Top 10 Searches" table on the overview tab
3. See at least one row with a query, count, and rank number
4. See the table sorted by count descending

## analytics-4: Searches tab detail view
1. Go to /index/e2e-products/analytics
2. Click the "Searches" tab
3. See the full Top Searches table with columns: #, Query, Count, Volume, Avg Hits
4. See the filter input for narrowing queries
5. Type a query in the filter input
6. See the table rows filtered client-side

## analytics-5: No results tab
1. Go to /index/e2e-products/analytics
2. Click the "No Results" tab
3. See the no-result rate banner showing a percentage
4. See the "Searches With No Results" table (or empty state if rate is 0%)

## analytics-6: Devices tab with platform breakdown
1. Go to /index/e2e-products/analytics
2. Click the "Devices" tab
3. See platform cards for Desktop, Mobile, and/or Tablet
4. See each card showing a count and percentage
5. See the "Searches by Device Over Time" chart (if data exists)

## analytics-7: Geography tab with country data
1. Go to /index/e2e-products/analytics
2. Click the "Geography" tab
3. See the Countries count card
4. See the "Searches by Country" table with country names and counts
5. Click a country row to drill down
6. See the drill-down view with "Top Searches from [Country]"
7. See the "All Countries" back button
8. Click "All Countries" to return to the country list

## analytics-8: Date range switching
1. Go to /index/e2e-products/analytics
2. See 7d selected in the date range selector
3. Click "30d"
4. See 30d become the active selection
5. See the KPI cards and charts update with new data
6. See the date range label update in the header
