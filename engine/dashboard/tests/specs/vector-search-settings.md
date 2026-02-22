# Vector Search Settings

Test index: `e2e-products` (pre-seeded)
Vector search settings are managed via the Settings page.
Backend: `GET/PUT /1/indexes/{indexName}/settings` handles `embedders`, `mode` fields.

## vector-settings-1: View search mode and embedders sections (load-and-verify)
1. Arrange: seed a userProvided embedder "default" (dims=384) via API
2. Go to /index/e2e-products/settings
3. See the "Search Mode" section heading
4. See the "Embedders" section heading
5. See the seeded embedder card showing name "default", source "userProvided", dimensions "384"

## vector-settings-2: Set search mode to Neural Search
1. Go to /index/e2e-products/settings
2. Find the "Search Mode" section
3. Select "Neural Search" from the mode dropdown
4. Click Save
5. Reload the page
6. Verify the mode dropdown still shows "Neural Search"

## vector-settings-3: Add userProvided embedder via dialog
1. Go to /index/e2e-products/settings
2. Click "Add Embedder" button
3. See the Add Embedder dialog open
4. Fill name "test-emb"
5. Select source "userProvided"
6. Fill dimensions 384
7. Click "Add Embedder" button in dialog
8. See new embedder card "test-emb" appear
9. Click Save settings
10. Reload page and verify "test-emb" card is still present

## vector-settings-4: Configure openAi embedder (API key masked)
1. Go to /index/e2e-products/settings
2. Click "Add Embedder"
3. Fill name "openai-emb"
4. Select source "openAi" (should be default)
5. See API Key input field with type "password"
6. See Model input field
7. Fill API Key "sk-test123"
8. Fill Model "text-embedding-3-small"
9. Click "Add Embedder" button in dialog
10. See new embedder card "openai-emb" appear with source badge "openAi"

## vector-settings-5: Delete an embedder
1. Arrange: seed embedder "to-delete" via API
2. Go to /index/e2e-products/settings
3. See embedder card "to-delete"
4. Click delete button on "to-delete" card
5. See confirmation dialog
6. Click "Confirm" in dialog
7. Verify "to-delete" card is no longer visible
8. Click Save settings
9. Reload and verify card is gone

## vector-settings-6: Embedder settings persist after save and reload
1. Go to /index/e2e-products/settings
2. Add an embedder via the dialog
3. Click Save settings
4. Navigate to a different page (e.g., search page)
5. Navigate back to settings
6. Verify the embedder card is still present
