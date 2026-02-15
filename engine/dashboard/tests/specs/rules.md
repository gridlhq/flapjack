# Rules Page

Test index: `e2e-products`
Pre-seeded rules:
- Pin MacBook Pro 16 when searching "laptop"
- Hide Galaxy Tab S9 when searching "tablet"

## rules-1: List existing rules (SMOKE)
1. Go to /index/e2e-products/rules
2. See breadcrumb "e2e-products / Rules"
3. See the rules count badge showing "2"
4. See the rules list with 2 entries
5. See each rule card showing its objectID
6. See the rule descriptions (e.g., "When query contains 'laptop', pin 1 result")
7. See green power icon on enabled rules

## rules-2: Create a new rule
1. Go to /index/e2e-products/rules
2. Click "Add Rule" button
3. See the rule editor dialog open with a JSON editor
4. Edit the JSON to set a pattern of "headphones" and add a promote consequence
5. Click "Create"
6. See the dialog close
7. See the new rule in the rules list
8. See the rules count badge increment
9. Cleanup: delete the created rule

## rules-3: Delete a rule
1. Go to /index/e2e-products/rules
2. Create a temporary rule via API
3. Refresh the page
4. See the temporary rule in the list
5. Click the trash icon on the temporary rule row
6. Confirm the deletion in the browser dialog
7. See the rule removed from the list

## rules-4: Rule badges show pin and hide counts
1. Go to /index/e2e-products/rules
2. Find the "laptop" pin rule
3. See a badge showing "1 pinned"
4. Find the "tablet" hide rule
5. See a badge showing "1 hidden"

## rules-5: Link to Merchandising Studio
1. Go to /index/e2e-products/rules
2. See the "Merchandising Studio" button in the header
3. Click "Merchandising Studio"
4. See the URL change to /index/e2e-products/merchandising
5. See the Merchandising Studio page load
