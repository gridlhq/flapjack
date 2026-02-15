# Overview Page

Test index: `e2e-products` (12 products pre-seeded)

## overview-1: Page loads with index data (SMOKE)
1. Go to /overview
2. See the "Overview" heading
3. See 4 stat cards: Indexes, Documents, Storage, Status
4. See the Indexes card showing at least "1"
5. See the Documents card showing "12"
6. See the Status card showing "Healthy" in green
7. See the index list with "e2e-products" visible
8. See the "e2e-products" row showing "12 documents"

## overview-2: Create a new index
1. Go to /overview
2. Click "Create Index" button
3. See the Create Index dialog open
4. Type "e2e-temp-index" in the Index Name field
5. Click "Create" button
6. See the dialog close
7. See "e2e-temp-index" appear in the index list
8. See the Indexes stat card increment by 1
9. Cleanup: delete "e2e-temp-index" via API

## overview-3: Delete an index
1. Go to /overview
2. Create a temporary index "e2e-delete-me" via API
3. Refresh the page
4. See "e2e-delete-me" in the index list
5. Click the trash icon on the "e2e-delete-me" row
6. See the confirmation dialog with text "e2e-delete-me"
7. Click "Delete" to confirm
8. See "e2e-delete-me" removed from the index list

## overview-4: Health indicator shows status
1. Go to /overview
2. See the Status stat card
3. See the value "Healthy" displayed in green text
4. See a green dot next to "e2e-products" in the index list

## overview-5: Index row navigates to search page
1. Go to /overview
2. Click the "e2e-products" row in the index list
3. See the URL change to /index/e2e-products
4. See the search page for "e2e-products" load
