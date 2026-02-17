# Synonyms Page

Test index: `e2e-products`
Pre-seeded synonyms:
- laptop = notebook = computer (multi-way)
- headphones = earphones = earbuds (multi-way)
- monitor = screen = display (multi-way)

## synonyms-1: List existing synonyms (SMOKE)
1. Go to /index/e2e-products/synonyms
2. See breadcrumb "e2e-products / Synonyms"
3. See the synonym count badge showing "3"
4. See the synonyms list with 3 entries
5. See "laptop = notebook = computer" displayed
6. See "headphones = earphones = earbuds" displayed
7. See "monitor = screen = display" displayed

## synonyms-2: Synonym type badges displayed
1. Go to /index/e2e-products/synonyms
2. See each synonym row has a type badge
3. See all 3 pre-seeded synonyms showing "Multi-way" badge

## synonyms-3: Create a multi-way synonym
1. Go to /index/e2e-products/synonyms
2. Click "Add Synonym" button
3. See the Create Synonym dialog open
4. See "Multi-way" type selected by default
5. Type "phone" in Word 1 field
6. Type "smartphone" in Word 2 field
7. Click "Add Word" to add a third word
8. Type "mobile" in Word 3 field
9. Click "Create"
10. See the dialog close
11. See the new synonym "phone = smartphone = mobile" in the list
12. See the synonym count badge increment to "4"
13. Cleanup: delete the created synonym

## synonyms-4: Delete a synonym
1. Go to /index/e2e-products/synonyms
2. Create a temporary synonym via API: "test1 = test2"
3. Refresh the page
4. See the temporary synonym in the list
5. Click the trash icon on the temporary synonym row
6. Confirm the deletion in the browser dialog
7. See the synonym removed from the list

## synonyms-5: Search synonyms
1. Go to /index/e2e-products/synonyms
2. See all 3 synonyms listed
3. Type "laptop" in the search box
4. See the list filtered to show only the laptop/notebook/computer synonym
5. Clear the search box
6. See all 3 synonyms listed again
