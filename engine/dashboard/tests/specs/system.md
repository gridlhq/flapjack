# System Page

## system-1: Health tab shows status (SMOKE)
1. Go to /system
2. See the "System" heading
3. See tabs: Health, Indexes, Replication, Snapshots
4. See the Health tab active by default
5. See "Auto-refreshes every 5 seconds" text
6. See the Status card showing "ok" with a green checkmark
7. See the Active Writers card showing writer counts
8. See the Facet Cache card showing cache stats
9. See the Index Health section with green dots for healthy indexes

## system-2: Indexes tab with index data
1. Go to /system
2. Click the "Indexes" tab
3. See summary cards: Total Indexes, Total Documents, Total Storage
4. See Total Indexes showing at least "1"
5. See Total Documents showing "12" (from e2e-products)
6. See the Index Details table
7. See "e2e-products" row with columns: Name, Status, Documents, Size, Pending
8. See Status column showing "Healthy" with green checkmark
9. See Documents column showing "12"

## system-3: Health auto-refresh
1. Go to /system
2. See the Health tab with current status values
3. Wait approximately 5 seconds
4. See the health data refresh (network request fires automatically)
5. See the status values remain consistent (no flicker to error state)

## system-4: Snapshots tab
1. Go to /system
2. Click the "Snapshots" tab
3. See the "Local Export / Import" section
4. See each index listed with Export and Import buttons
5. See the "S3 Backups" section (may show "not configured" message)
