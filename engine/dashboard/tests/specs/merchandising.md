# Merchandising Studio

Test index: `e2e-products` (12 products pre-seeded)

## merchandising-1: Search and see results (SMOKE)
1. Go to /index/e2e-products/merchandising
2. See the "Merchandising Studio" heading
3. See the search input with placeholder text
4. Type "laptop" in the search box
5. Click "Search" button (or press Enter)
6. See search results displayed as cards
7. See result count and processing time
8. See each card with an objectID badge, Pin button, and Hide button

## merchandising-2: Pin a result to position
1. Go to /index/e2e-products/merchandising
2. Search for "laptop"
3. See results listed
4. Click the Pin button on the second result
5. See the result highlighted with a blue border
6. See a "Pinned #N" badge on the pinned result
7. See the header show "1 pinned, 0 hidden" badge
8. See the "Save as Rule" button appear
9. See the "Reset" button appear

## merchandising-3: Hide a result
1. Go to /index/e2e-products/merchandising
2. Search for "laptop"
3. See results listed
4. Click the Hide button on a result
5. See the result removed from the main results list
6. See a "Hidden Results" section appear below the results
7. See the hidden result shown with strikethrough text
8. See the header badge update to show "0 pinned, 1 hidden"

## merchandising-4: Save as merchandising rule
1. Go to /index/e2e-products/merchandising
2. Search for "test-merch-query"
3. Pin a result to a position
4. Hide another result
5. See the "Save as Rule" button enabled
6. Click "Save as Rule"
7. See a success toast message "Merchandising rule saved"
8. See the pin and hide state reset after saving
9. Go to /index/e2e-products/rules
10. See the new merchandising rule in the rules list
11. Cleanup: delete the created rule

## merchandising-5: Reset merchandising changes
1. Go to /index/e2e-products/merchandising
2. Search for "laptop"
3. Pin a result and hide another result
4. See the header show changes badge
5. Click "Reset" button
6. See all pins and hides cleared
7. See results return to their original order
8. See the "Save as Rule" and "Reset" buttons disappear
