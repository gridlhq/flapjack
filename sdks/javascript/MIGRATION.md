# Migration Guide: Algolia to Flapjack Search

This guide covers switching a JavaScript/TypeScript application from `algoliasearch` to `flapjack-search`. The Flapjack JS SDK is a drop-in replacement for Algolia's v5 client — same API surface, same types, same InstantSearch compatibility.

## 1. Install the Flapjack SDK

```bash
# Remove Algolia
npm uninstall algoliasearch

# Install Flapjack
npm install flapjack-search@beta
```

The `flapjack-search` package replaces `algoliasearch`. All sub-packages (`@flapjack-search/client-common`, `@flapjack-search/client-search`, etc.) are installed automatically as dependencies.

## 2. Update imports

### Standard client

```diff
- import algoliasearch from 'algoliasearch';
+ import { flapjackSearch } from 'flapjack-search';

- const client = algoliasearch('APP_ID', 'API_KEY');
+ const client = flapjackSearch('APP_ID', 'API_KEY');
```

### Lite client (search-only, smaller bundle)

> **Note:** `flapjack-search/lite` is not yet available. Use the main `flapjack-search` import for now — it works in all the same contexts. A dedicated lite build is planned for a future release.

```diff
- import { liteClient } from 'algoliasearch/lite';
+ import { flapjackSearch } from 'flapjack-search';

- const client = liteClient('APP_ID', 'API_KEY');
+ const client = flapjackSearch('APP_ID', 'API_KEY');
```

### Self-hosted Flapjack server

If you're running Flapjack on your own infrastructure instead of Flapjack Cloud:

```js
import { flapjackSearch } from 'flapjack-search';

const client = flapjackSearch('my-app', 'my-api-key', {
  hosts: [{ url: 'search.example.com', protocol: 'https', accept: 'readWrite' }],
});
```

For local development:

```js
const client = flapjackSearch('test-app', 'test-api-key', {
  hosts: [{ url: 'localhost:7700', protocol: 'http', accept: 'readWrite' }],
});
```

## 3. Framework integrations — no changes needed

The Flapjack client implements the same `SearchClient` interface that Algolia's InstantSearch ecosystem expects. You only need to change the import and initialization — all widget/component code stays the same.

### InstantSearch.js

```diff
- import algoliasearch from 'algoliasearch';
+ import { flapjackSearch } from 'flapjack-search';
  import instantsearch from 'instantsearch.js';
  import { searchBox, hits } from 'instantsearch.js/es/widgets';

- const client = algoliasearch('APP_ID', 'API_KEY');
+ const client = flapjackSearch('APP_ID', 'API_KEY');

  // Everything below is unchanged
  const search = instantsearch({ searchClient: client, indexName: 'products' });
  search.addWidgets([searchBox({ container: '#search' }), hits({ container: '#hits' })]);
  search.start();
```

### React InstantSearch

```diff
- import algoliasearch from 'algoliasearch';
+ import { flapjackSearch } from 'flapjack-search';
  import { InstantSearch, SearchBox, Hits } from 'react-instantsearch';

- const client = algoliasearch('APP_ID', 'API_KEY');
+ const client = flapjackSearch('APP_ID', 'API_KEY');

  // JSX is unchanged
  <InstantSearch searchClient={client} indexName="products">
    <SearchBox />
    <Hits />
  </InstantSearch>
```

### Vue InstantSearch

```diff
- import algoliasearch from 'algoliasearch';
+ import { flapjackSearch } from 'flapjack-search';
  import InstantSearch from 'vue-instantsearch/vue3/es';

- const client = algoliasearch('APP_ID', 'API_KEY');
+ const client = flapjackSearch('APP_ID', 'API_KEY');

  // Template is unchanged
  <ais-instant-search :search-client="client" index-name="products">
    <ais-search-box />
    <ais-hits />
  </ais-instant-search>
```

### Autocomplete.js

```diff
- import algoliasearch from 'algoliasearch';
+ import { flapjackSearch } from 'flapjack-search';
  import { autocomplete, getAlgoliaResults } from '@algolia/autocomplete-js';

- const client = algoliasearch('APP_ID', 'API_KEY');
+ const client = flapjackSearch('APP_ID', 'API_KEY');

  // Autocomplete setup is unchanged — getAlgoliaResults works with the Flapjack client
  autocomplete({
    container: '#autocomplete',
    getSources({ query }) {
      return [{
        sourceId: 'products',
        getItems() {
          return getAlgoliaResults({ searchClient: client, queries: [{ indexName: 'products', query }] });
        },
      }];
    },
  });
```

## 4. API method mapping

Every method on the Algolia client has an identical counterpart on the Flapjack client. No code changes needed.

| Operation | Method | Unchanged? |
|-----------|--------|-----------|
| Search | `client.search(queries)` | Yes |
| Get object | `client.getObject({ indexName, objectID })` | Yes |
| Save objects | `client.saveObjects({ indexName, objects })` | Yes |
| Delete object | `client.deleteObject({ indexName, objectID })` | Yes |
| Partial update | `client.partialUpdateObject({ indexName, objectID, ... })` | Yes |
| Batch | `client.batch({ indexName, batchWriteParams })` | Yes |
| Get settings | `client.getSettings({ indexName })` | Yes |
| Set settings | `client.setSettings({ indexName, indexSettings })` | Yes |
| Save synonyms | `client.saveSynonyms({ indexName, synonymHit })` | Yes |
| Save rules | `client.saveRules({ indexName, rules })` | Yes |
| List indices | `client.listIndices()` | Yes |
| Delete index | `client.deleteIndex({ indexName })` | Yes |
| Browse | `client.browse({ indexName })` | Yes |
| Wait for task | `client.waitForTask({ indexName, taskID })` | Yes |
| API keys | `client.addApiKey()` / `getApiKey()` / `deleteApiKey()` | Yes |
| Facet search | `client.searchForFacetValues(...)` | Yes |

## 5. TypeScript types

Types are exported from the same package paths:

```diff
- import type { SearchResponse, Hit } from 'algoliasearch';
+ import type { SearchResponse, Hit } from 'flapjack-search';
```

Or from the lower-level package:

```diff
- import type { SearchResponse } from '@algolia/client-search';
+ import type { SearchResponse } from '@flapjack-search/client-search';
```

## 6. Algolia agent strings

If you use `addAlgoliaAgent()` for analytics or user-agent tracking, it still works:

```js
// Both work identically
client.addAlgoliaAgent('MyApp', '1.0.0');
client.addFlapjackAgent('MyApp', '1.0.0');
```

## 7. Server-side differences

When migrating from Algolia Cloud to a self-hosted Flapjack server:

| Feature | Algolia Cloud | Flapjack Self-Hosted |
|---------|--------------|---------------------|
| Hosting | Managed SaaS | Your infrastructure |
| Host config | Automatic (APP_ID-based DNS) | Manual via `hosts` option |
| API keys | Managed via dashboard | Configured at server startup |
| Replicas | Native support | Not yet supported |
| A/B Testing | Native support | Not yet supported |
| Analytics | Native support | Not yet supported |
| Personalization | Native support | Not yet supported |

## 8. What doesn't need to change

- **All InstantSearch widgets** — searchBox, hits, refinementList, pagination, stats, menu, currentRefinements, highlight, snippet, etc.
- **All React InstantSearch hooks** — useSearchBox, useHits, useRefinementList, usePagination, useStats, useInstantSearch, useConfigure, etc.
- **All Vue InstantSearch components** — ais-search-box, ais-hits, ais-refinement-list, ais-pagination, ais-stats, ais-configure, ais-highlight, ais-menu, etc.
- **Autocomplete.js** — getAlgoliaResults, fetchAlgoliaResults, getAlgoliaFacets all work unchanged
- **algoliasearch-helper** — works as-is with the Flapjack client
- **Search parameters** — filters, facets, hitsPerPage, page, attributesToRetrieve, etc.
- **Highlighting** — _highlightResult, _snippetResult, pre/post tags

## 9. Quick migration checklist

- [ ] `npm uninstall algoliasearch && npm install flapjack-search@beta`
- [ ] Find/replace `import algoliasearch from 'algoliasearch'` → `import { flapjackSearch } from 'flapjack-search'`
- [ ] Find/replace `import { liteClient } from 'algoliasearch/lite'` → `import { flapjackSearch } from 'flapjack-search'` (lite build not yet available)
- [ ] Find/replace `algoliasearch(` → `flapjackSearch(`
- [ ] Update TypeScript type imports if using `@algolia/client-search` → `@flapjack-search/client-search`
- [ ] If self-hosted: add `hosts` config to client initialization
- [ ] Run your test suite — everything should pass without further changes
