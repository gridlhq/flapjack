# Hybrid Search Controls

Test index: `e2e-products` (pre-seeded)
Hybrid search controls appear on the Search page when embedders are configured.
Backend: `POST /1/indexes/{indexName}/query` handles `hybrid: { semanticRatio, embedder }` in request body.

## hybrid-search-1: Hybrid controls hidden when no embedders configured
1. Arrange: ensure no embedders on test index (clear via API)
2. Go to /index/e2e-products
3. Wait for search results to load
4. Verify the hybrid controls bar is NOT visible

## hybrid-search-2: Hybrid controls visible when embedders configured
1. Arrange: seed userProvided embedder "default" (dims=384) via API
2. Go to /index/e2e-products
3. Wait for search results to load
4. See the "Hybrid Search" label
5. See the semantic ratio slider
6. See the ratio label (default: "Balanced")

## hybrid-search-3: Adjust semantic ratio slider and verify label updates
1. Arrange: seed userProvided embedder via API
2. Go to /index/e2e-products
3. See the hybrid controls bar
4. Change the slider value toward semantic end
5. See the ratio label update (e.g., "70% semantic" or "Semantic only")

## hybrid-search-4: Search results appear when hybrid search is active
1. Arrange: seed userProvided embedder + documents with _vectors via API
2. Go to /index/e2e-products
3. See hybrid controls
4. Type a search query
5. Verify results appear in the results panel
