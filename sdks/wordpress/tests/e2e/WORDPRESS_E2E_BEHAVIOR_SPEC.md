# WordPress E2E Behavior Specification

**Last Updated:** 2026-02-10
**Total Behaviors:** 35
**Test Coverage:** 100%

This document defines all user-facing behaviors that must be tested in E2E tests. Each behavior maps to one or more test cases.

---

## 1. InstantSearch Overlay (4 behaviors)

### BEH-IS-001: Overlay opens on search icon click
**Feature:** InstantSearch Overlay
**User Story:** As a site visitor, when I click the search icon, I want the search overlay to appear immediately with the input focused.

**Preconditions:**
- Plugin configured with valid API credentials
- InstantSearch feature enabled in settings
- User on any front-end page

**Actions:**
1. User clicks search icon/trigger

**Expected Outcomes:**
- ✅ Full-screen overlay appears with fade-in animation
- ✅ Search input has keyboard focus automatically
- ✅ Placeholder text displays (e.g., "Search...")
- ✅ Close button is visible

**Edge Cases:**
- Multiple clicks should not create multiple overlays
- Works on mobile viewports

---

### BEH-IS-002: Search results update in real-time
**Feature:** InstantSearch Real-time Results
**User Story:** As a user typing in the search box, I want to see results appear instantly as I type, without pressing Enter.

**Preconditions:**
- InstantSearch overlay is open
- Search input is focused

**Actions:**
1. User types search query (e.g., "test")
2. User continues typing (e.g., "test product")

**Expected Outcomes:**
- ✅ Results appear within 300ms of typing
- ✅ Each result shows: title, excerpt, thumbnail (if available)
- ✅ Results update dynamically as typing continues
- ✅ "Powered by" logo displayed (if configured)

**Edge Cases:**
- Empty query shows no results or recent searches
- Special characters handled correctly
- Very long queries truncated appropriately

---

### BEH-IS-003: Clicking result navigates to page
**Feature:** Search Result Navigation
**User Story:** As a user viewing search results, when I click a result, I want to navigate to that page/post.

**Preconditions:**
- Search results are displayed
- At least one result is visible

**Actions:**
1. User clicks on first search result

**Expected Outcomes:**
- ✅ Browser navigates to clicked post/page URL
- ✅ Overlay closes automatically
- ✅ Correct post content displays on destination page

**Edge Cases:**
- Middle-click opens in new tab
- Cmd/Ctrl-click opens in new tab
- Back button returns to previous page

---

### BEH-IS-004: ESC key closes overlay
**Feature:** Keyboard Accessibility
**User Story:** As a keyboard user, I want to press ESC to close the search overlay and return focus to where I was.

**Preconditions:**
- Overlay is open

**Actions:**
1. User presses ESC key

**Expected Outcomes:**
- ✅ Overlay closes with fade-out animation
- ✅ Focus returns to search icon trigger
- ✅ Search input value persists if user reopens overlay

**Edge Cases:**
- Multiple ESC presses don't cause errors
- Works consistently across browsers

---

## 2. WooCommerce Faceted Search (8 behaviors)

### BEH-WC-001: Facets render on product archive pages
**Feature:** WooCommerce Integration
**User Story:** As a shopper on the shop page, I want to see filter options to narrow down products.

**Preconditions:**
- WooCommerce plugin active
- Products indexed in Algolia/Flapjack
- Facets enabled in plugin settings

**Actions:**
1. User navigates to /shop/ page

**Expected Outcomes:**
- ✅ Facet widgets display (categories, attributes, price)
- ✅ Facets show current item count for each option
- ✅ Checkboxes or sliders render correctly

**Edge Cases:**
- Works with custom WooCommerce themes
- Facets hide gracefully if no products

---

### BEH-WC-002: Clicking facet filters products
**Feature:** Product Filtering
**User Story:** As a shopper, when I select a facet, I want the product grid to update to show only matching products.

**Preconditions:**
- On shop page with facets visible

**Actions:**
1. User clicks first available facet checkbox

**Expected Outcomes:**
- ✅ Product grid updates to show filtered results
- ✅ Facet checkbox shows checked state
- ✅ URL updates with facet parameter (for sharing/bookmarking)

**Edge Cases:**
- Filter with zero results shows "No products found"
- Animation/loading indicator during filter

---

### BEH-WC-003: Multiple facets combine with AND logic
**Feature:** Multi-facet Filtering
**User Story:** As a shopper, I want to apply multiple filters to narrow down products (e.g., "Blue" AND "Large").

**Preconditions:**
- On shop page with multiple facets

**Actions:**
1. User selects first facet
2. User selects second facet from different attribute

**Expected Outcomes:**
- ✅ Products match BOTH selected facets (AND logic)
- ✅ Product count updates to reflect combined filters
- ✅ URL contains both facet parameters

**Edge Cases:**
- Three or more facets work correctly
- No results scenario handled gracefully

---

### BEH-WC-004: Clearing facet restores all products
**Feature:** Filter Reset
**User Story:** As a shopper, I want to clear my filters to see all products again.

**Preconditions:**
- At least one facet applied

**Actions:**
1. User clicks "Clear Filters" button OR unchecks all facets

**Expected Outcomes:**
- ✅ All facets reset to unchecked
- ✅ Product count returns to original total
- ✅ URL returns to /shop/ (no query parameters)

**Edge Cases:**
- Partial clear (unchecking one of multiple facets)
- Clear persists after page reload

---

### BEH-WC-005: URL updates with facet selections
**Feature:** Shareable Filters
**User Story:** As a shopper, I want to share or bookmark filtered product pages via URL.

**Preconditions:**
- Facets applied

**Actions:**
1. User applies facets
2. User copies URL and opens in new tab/browser

**Expected Outcomes:**
- ✅ New tab loads with same filters applied
- ✅ Product grid matches original filtered view
- ✅ Facet checkboxes reflect applied filters

**Edge Cases:**
- URL parameters work after page refresh
- Invalid facet values in URL handled gracefully

---

### BEH-WC-006: Facet counts update dynamically
**Feature:** Dynamic Facet Counts
**User Story:** As a shopper, when I apply a filter, I want to see updated counts showing how many products remain in each facet option.

**Preconditions:**
- Multiple facets available

**Actions:**
1. User applies first facet

**Expected Outcomes:**
- ✅ Facet counts update to reflect filtered results
- ✅ Options with zero results show (0) or hide (depending on configuration)
- ✅ Counts are accurate

**Edge Cases:**
- Counts update smoothly without flicker
- Zero-result facets behavior configurable

---

### BEH-WC-007: Price slider filters correctly
**Feature:** Price Range Filter
**User Story:** As a shopper, I want to filter products by price range using a slider.

**Preconditions:**
- Price facet/slider available on shop page

**Actions:**
1. User adjusts price slider (e.g., max price to $100)

**Expected Outcomes:**
- ✅ Products filtered to show only items within price range
- ✅ URL updates with price parameters
- ✅ Slider values display current min/max

**Edge Cases:**
- Min price higher than max price prevented
- Currency formatting correct
- Works with different currencies

---

### BEH-WC-008: Out-of-stock products excluded when configured
**Feature:** Stock Filtering
**User Story:** As a shopper, I don't want to see out-of-stock products (if setting enabled).

**Preconditions:**
- Setting "Hide out of stock" enabled
- Some products marked as out of stock

**Actions:**
1. User views shop page

**Expected Outcomes:**
- ✅ Out-of-stock products do NOT appear in results
- ✅ Facet counts exclude out-of-stock products
- ✅ Search results respect stock status

**Edge Cases:**
- Product transitions from in-stock to out-of-stock
- Works with product variations

---

## 3. Settings Page UI (10 behaviors)

### BEH-SET-001: API key validation and save
**Feature:** API Configuration
**User Story:** As an admin, I want to enter and save my Flapjack API credentials.

**Actions:**
1. Enter valid Application ID
2. Enter valid Admin API Key
3. Click Save

**Expected Outcomes:**
- ✅ Success message displays
- ✅ Settings persist after page reload

---

### BEH-SET-002: Application ID validation
**Feature:** Field Validation
**User Story:** As an admin, I should see an error if I forget to enter required API credentials.

**Actions:**
1. Leave Application ID empty
2. Click Save

**Expected Outcomes:**
- ✅ Error message displays
- ✅ Settings NOT saved
- ✅ Field highlighted as invalid

---

### BEH-SET-003: Search-Only API Key validation
**Feature:** Security
**User Story:** As an admin, I want to configure a search-only API key for frontend use.

**Actions:**
1. Enter Search-Only API Key
2. Click "Test Connection" (if available)

**Expected Outcomes:**
- ✅ Success or error message based on key validity
- ✅ Help text explains key should be search-only (no write permissions)

---

### BEH-SET-004: Index prefix configuration
**Feature:** Multi-environment Support
**User Story:** As an admin, I want to set an index prefix for staging/production separation.

**Actions:**
1. Enter index prefix (e.g., "staging_")
2. Click Save

**Expected Outcomes:**
- ✅ Settings save successfully
- ✅ Prefix persists after reload
- ✅ New indices created with prefix

---

### BEH-SET-005: Instant Search enable/disable toggle
**Feature:** Feature Toggle
**User Story:** As an admin, I want to enable/disable InstantSearch overlay.

**Actions:**
1. Toggle InstantSearch checkbox
2. Click Save

**Expected Outcomes:**
- ✅ Toggle state persists
- ✅ Frontend reflects change (overlay appears or doesn't)
- ✅ No errors on toggle

---

### BEH-SET-006: Facet attribute selection
**Feature:** WooCommerce Facet Configuration
**User Story:** As an admin, I want to choose which product attributes appear as facets.

**Actions:**
1. Check 2-3 facet attributes (e.g., Color, Size)
2. Click Save

**Expected Outcomes:**
- ✅ Selections save successfully
- ✅ Selected facets appear on shop page
- ✅ Unselected facets do NOT appear

---

### BEH-SET-007: Reindex button triggers indexing
**Feature:** Manual Reindex
**User Story:** As an admin, I want to manually trigger a full reindex of my content.

**Actions:**
1. Click "Reindex" button

**Expected Outcomes:**
- ✅ Progress bar displays
- ✅ Button disabled during indexing
- ✅ Success message shows item count when complete

---

### BEH-SET-008: Index status displays correctly
**Feature:** Index Monitoring
**User Story:** As an admin, I want to see when my content was last indexed and how many items are indexed.

**Expected Outcomes:**
- ✅ Shows last indexed date/time
- ✅ Displays record count
- ✅ Indicates if index is outdated

---

### BEH-SET-009: Error messages display for invalid inputs
**Feature:** Error Handling
**User Story:** As an admin, I should see clear error messages for invalid inputs.

**Actions:**
1. Leave required fields empty
2. Click Save

**Expected Outcomes:**
- ✅ Error message appears above form
- ✅ Form does NOT submit
- ✅ Error is actionable (tells user what to fix)

---

### BEH-SET-010: Success messages display on save
**Feature:** User Feedback
**User Story:** As an admin, I want confirmation that my settings saved successfully.

**Actions:**
1. Update any setting
2. Click Save

**Expected Outcomes:**
- ✅ Green success banner displays
- ✅ Message contains confirmation text ("Settings saved")
- ✅ Message auto-dismisses after 3-5 seconds (optional)

---

## 4. Gutenberg Block Editor (3 behaviors)

### BEH-GB-001: Search block appears in block inserter
**Feature:** Gutenberg Integration
**User Story:** As a content editor, I want to find and insert a Flapjack Search block.

**Preconditions:**
- Editing post/page in Gutenberg

**Actions:**
1. Click "+" to open block inserter
2. Search for "Flapjack"

**Expected Outcomes:**
- ✅ "Flapjack Search" block appears in results
- ✅ Block has Algolia/Flapjack icon
- ✅ Clicking block inserts it into editor

---

### BEH-GB-002: Block renders search input in editor
**Feature:** Block Preview
**User Story:** As an editor, I want to see a preview of the search block in the editor.

**Preconditions:**
- Search block inserted

**Expected Outcomes:**
- ✅ Search input placeholder displays in editor
- ✅ Block has outline indicating selection
- ✅ Preview matches frontend appearance

---

### BEH-GB-003: Block settings panel configures placeholder
**Feature:** Block Customization
**User Story:** As an editor, I want to customize the search placeholder text.

**Preconditions:**
- Search block selected

**Actions:**
1. Open block settings sidebar
2. Change placeholder text to "Search products..."

**Expected Outcomes:**
- ✅ Preview updates to show new placeholder
- ✅ Change persists on publish
- ✅ Frontend displays custom placeholder

---

## 5. Backend Search UI (3 behaviors)

### BEH-BE-001: Admin search shows InstantSearch results
**Feature:** Admin Dashboard Search
**User Story:** As an admin, I want to use InstantSearch in the WordPress admin dashboard.

**Preconditions:**
- Logged in as admin
- Backend InstantSearch enabled

**Actions:**
1. Click admin search bar
2. Type query

**Expected Outcomes:**
- ✅ Dropdown shows InstantSearch results
- ✅ Results include posts, pages, products
- ✅ Results update in real-time

---

### BEH-BE-002: Backend autocomplete displays suggestions
**Feature:** Admin Autocomplete
**User Story:** As an admin, I want autocomplete suggestions when searching in the backend.

**Actions:**
1. Type partial query in admin search

**Expected Outcomes:**
- ✅ Suggestions include matching terms
- ✅ Query highlighting shows matched portion
- ✅ Up/down arrows navigate suggestions

---

### BEH-BE-003: Search analytics tracked in backend
**Feature:** Analytics Dashboard
**User Story:** As an admin, I want to see which search terms users are entering.

**Preconditions:**
- Analytics enabled
- Some searches performed

**Actions:**
1. Navigate to Analytics page

**Expected Outcomes:**
- ✅ Page shows top searches
- ✅ Displays search count
- ✅ Shows click-through rate (if applicable)

---

## 6. Autocomplete (3 behaviors)

### BEH-AC-001: Autocomplete displays suggestions on typing
**Feature:** Search Autocomplete
**User Story:** As a user, I want to see autocomplete suggestions as I type in the search box.

**Preconditions:**
- Frontend with search widget
- Autocomplete enabled

**Actions:**
1. Click search input
2. Type "prod"

**Expected Outcomes:**
- ✅ Dropdown appears with suggestions
- ✅ Shows recent queries + top results
- ✅ Suggestions include product names, post titles

---

### BEH-AC-002: Arrow keys navigate suggestions
**Feature:** Keyboard Navigation
**User Story:** As a keyboard user, I want to navigate autocomplete suggestions with arrow keys.

**Actions:**
1. Type query to show suggestions
2. Press Down Arrow 3 times

**Expected Outcomes:**
- ✅ Third suggestion highlighted
- ✅ Highlighted suggestion has blue/active background
- ✅ Search input updates with highlighted text

---

### BEH-AC-003: Enter key submits selected suggestion
**Feature:** Quick Navigation
**User Story:** As a user, I want to press Enter to navigate to the highlighted suggestion.

**Actions:**
1. Type query
2. Navigate to second suggestion with arrow keys
3. Press Enter

**Expected Outcomes:**
- ✅ Browser navigates to selected result OR shows search results
- ✅ Autocomplete closes

---

## 7. Activation and Setup Flows (4 behaviors)

### BEH-ACT-001: Plugin activation redirects to settings
**Feature:** Onboarding
**User Story:** As an admin, when I activate the plugin, I want to be guided to the settings page.

**Preconditions:**
- Plugin uploaded but not activated

**Actions:**
1. Activate plugin from Plugins page

**Expected Outcomes:**
- ✅ Redirects to settings page OR shows activation notice
- ✅ Welcome message displays

---

### BEH-ACT-002: Setup wizard guides through configuration
**Feature:** Guided Setup
**User Story:** As a first-time user, I want a setup wizard to guide me through configuration.

**Preconditions:**
- First-time activation

**Expected Outcomes:**
- ✅ Progress indicator shows steps (1/4, 2/4, etc.)
- ✅ Each step validates before proceeding
- ✅ Final step triggers initial indexing

---

### BEH-ACT-003: First-time indexing completes successfully
**Feature:** Initial Index
**User Story:** As an admin, I want my content indexed automatically after setup.

**Actions:**
1. Complete setup wizard OR click Reindex

**Expected Outcomes:**
- ✅ Progress bar reaches 100%
- ✅ Success message shows item count
- ✅ Search immediately functional

---

### BEH-ACT-004: Deactivation preserves settings
**Feature:** Data Persistence
**User Story:** As an admin, I want my settings preserved if I temporarily deactivate the plugin.

**Actions:**
1. Configure plugin with settings
2. Deactivate plugin
3. Reactivate plugin

**Expected Outcomes:**
- ✅ All settings preserved
- ✅ Indices remain in Algolia/Flapjack
- ✅ No data loss on reactivation

---

## Test Coverage Summary

| Feature Area | Behaviors | Tests | Coverage |
|-------------|-----------|-------|----------|
| InstantSearch Overlay | 4 | 4 | 100% |
| WooCommerce Facets | 8 | 8 | 100% |
| Settings Page | 10 | 10 | 100% |
| Gutenberg Block | 3 | 3 | 100% |
| Backend Search | 3 | 3 | 100% |
| Autocomplete | 3 | 3 | 100% |
| Activation/Setup | 4 | 4 | 100% |
| **TOTAL** | **35** | **35** | **100%** |

---

## Test Execution Notes

### Local Testing
- Set `TEST_MODE=local` to use mock Flapjack API responses
- Requires WordPress installation at http://localhost:8080 (or configured URL)
- Admin credentials: username=`admin`, password=`password`

### CI Testing
- Tests run against real Flapjack API in staging environment
- Uses test credentials from environment variables
- Playwright browsers: Chromium, Firefox, WebKit

### Test Data Requirements
- At least 10 published posts
- At least 5 WooCommerce products (if testing facets)
- Mix of in-stock and out-of-stock products
- Products with multiple attributes (Color, Size, etc.)

---

## Maintenance

This behavior spec should be updated whenever:
1. New user-facing features are added
2. Existing behaviors change
3. Edge cases are discovered
4. Test coverage gaps are identified

**Version History:**
- v1.0.0 (2026-02-10): Initial comprehensive behavior spec covering all 35 E2E tests
