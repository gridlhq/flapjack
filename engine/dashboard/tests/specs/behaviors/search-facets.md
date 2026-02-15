# Search Facets — Tier 1: BDD Behavior Specifications (Extended)

## B-SEARCH-005: Facets Remain Visible When Filter Produces 0 Results

**As a** user filtering search results
**I want to** see facets even when my filter combination produces zero results
**So that** I can adjust my filters without having to start over

**Acceptance Criteria:**
- When a facet filter combination yields 0 search results, the facets panel still displays all available facet attributes
- The "No facets configured" message is ONLY shown when the index truly has no facets configured in settings
- Active filter chips remain visible so the user can remove them
- The "Clear" button is available to reset all filters

---

## B-SEARCH-006: Facet Counts Reflect Current Search

**As a** user browsing faceted search results
**I want to** see facet value counts that match my current search/filter state
**So that** I can make informed filtering decisions

**Acceptance Criteria:**
- Facet value counts update when the search query changes
- Facet value counts update when facet filters are applied
- Counts shown next to each facet value reflect the number of documents matching the current query+filters that have that facet value
- If a search returns 1 result, facet counts should reflect that 1 result (not show unfiltered totals)
