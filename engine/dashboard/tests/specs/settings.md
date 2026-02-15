# Settings Page

Test index: `e2e-products`
- searchableAttributes: name, description, brand, category, tags
- facets: category, brand, filterOnly(price), filterOnly(inStock)

## settings-1: View settings form (SMOKE)
1. Go to /index/e2e-products/settings
2. See breadcrumb showing "e2e-products / Settings"
3. See the settings form loaded (not skeleton loaders)
4. See the "JSON" toggle button in the header
5. See the "Compact Index" button in the header

## settings-2: Searchable attributes displayed
1. Go to /index/e2e-products/settings
2. Find the "Searchable Attributes" section
3. See the following attributes listed: name, description, brand, category, tags
4. See them displayed in their priority order

## settings-3: Facets configuration displayed
1. Go to /index/e2e-products/settings
2. Find the "Faceting" or "Attributes for Faceting" section
3. See the following facets listed: category, brand, filterOnly(price), filterOnly(inStock)

## settings-4: Save settings changes
1. Go to /index/e2e-products/settings
2. Make a change to a setting (e.g., toggle a switch or modify a field)
3. See the "Save Changes" button appear
4. See the "Reset" button appear
5. Click "Save Changes"
6. See the button show "Saving..." while in progress
7. See the changes saved (button disappears after success)
8. Refresh the page and verify the change persisted
9. Cleanup: restore original settings

## settings-5: JSON editor view toggle
1. Go to /index/e2e-products/settings
2. Click the "JSON" toggle button
3. See the form replaced by a JSON editor (Monaco editor)
4. See the settings displayed as formatted JSON
5. See searchableAttributes array in the JSON
6. Click the "JSON" toggle button again
7. See the form view restored
