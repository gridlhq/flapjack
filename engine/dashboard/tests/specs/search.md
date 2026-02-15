# Search Page

Test index: `e2e-products` (12 products pre-seeded)
- Categories: Laptops, Tablets, Audio, Storage, Monitors, Accessories
- Brands: Apple, Lenovo, Dell, Samsung, Sony, LG, Logitech, Keychron, CalDigit
- Facets: category, brand, filterOnly(price), filterOnly(inStock)
- Synonyms: laptop/notebook/computer, headphones/earphones/earbuds, monitor/screen/display
- Rules: pin MacBook when "laptop", hide Galaxy Tab when "tablet"

## search-1: Basic search (SMOKE)
1. Go to /index/e2e-products
2. See the search box and index name "e2e-products"
3. Type "laptop" in the search box
4. Press Enter
5. See results including "MacBook Pro 16"
6. See MacBook Pro 16 pinned as the first result (due to rule)
7. See result count displayed

## search-2: Filter by category facet
1. Go to /index/e2e-products
2. Wait for the facets panel to load on the right side
3. See "category" facet with values listed
4. Click "Audio" in the category facet
5. See only Audio products in the results
6. See result count decrease

## search-3: Filter by brand facet
1. Go to /index/e2e-products
2. Wait for facets panel to load
3. See "brand" facet with values listed
4. Click "Apple" in the brand facet
5. See only Apple products in the results
6. See result count update to match Apple product count

## search-4: Clear facet filters
1. Go to /index/e2e-products
2. Click "Audio" in the category facet
3. See filtered results (only Audio products)
4. Click the active "Audio" filter to deselect it
5. See results return to the full unfiltered set
6. See result count return to original value

## search-5: Pagination through results
1. Go to /index/e2e-products
2. Clear the search box (empty query returns all 12 products)
3. See results displayed (up to hitsPerPage limit)
4. If more than one page, see pagination controls
5. Click "Next" to go to page 2
6. See a different set of results on page 2
7. Click "Previous" to return to page 1

## search-6: Empty search results
1. Go to /index/e2e-products
2. Type "xyznonexistent123" in the search box
3. Press Enter
4. See a "No results" or empty state message
5. See result count showing 0

## search-7: Synonym search
1. Go to /index/e2e-products
2. Type "notebook" in the search box
3. Press Enter
4. See laptop products in the results (synonym: notebook = laptop)
5. See "MacBook Pro 16" in the results

## search-8: Add documents dialog
1. Go to /index/e2e-products
2. Click "Add Documents" button
3. See the Add Documents dialog open
4. See options for adding documents (JSON editor, sample data, etc.)
5. Close the dialog without adding anything
