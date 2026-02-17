# Migration Guide: algoliasearch to flapjack-search (Python)

This guide covers switching a Python application from `algoliasearch` to `flapjack-search`. The Flapjack Python SDK is a drop-in replacement for Algolia's v4 client — same API surface, same types, same method signatures.

## 1. Install

```bash
pip uninstall algoliasearch
pip install flapjack-search
```

## 2. Update imports

```diff
- from algoliasearch.search.client import SearchClientSync
- from algoliasearch.search.client import SearchClient
- from algoliasearch.search.config import SearchConfig
- from algoliasearch.search.models import SearchMethodParams, SearchForHits, SearchQuery
+ from flapjacksearch.search.client import SearchClientSync
+ from flapjacksearch.search.client import SearchClient
+ from flapjacksearch.search.config import SearchConfig
+ from flapjacksearch.search.models import SearchMethodParams, SearchForHits, SearchQuery
```

The pattern is simple: replace `algoliasearch` with `flapjacksearch` in all imports.

## 3. Self-hosted server configuration

If you're running Flapjack on your own infrastructure:

```python
from flapjacksearch.search.client import SearchClientSync
from flapjacksearch.search.config import SearchConfig
from flapjacksearch.http.hosts import Host, HostsCollection, CallType

config = SearchConfig("my-app", "my-api-key")
config.hosts = HostsCollection([
    Host(url="search.example.com", scheme="https", accept=CallType.READ | CallType.WRITE)
])
client = SearchClientSync.create_with_config(config=config)
```

For local development:

```python
config = SearchConfig("test-app", "test-api-key")
config.hosts = HostsCollection([
    Host(url="localhost:7700", scheme="http", accept=CallType.READ | CallType.WRITE)
])
client = SearchClientSync.create_with_config(config=config)
```

## 4. Environment variables

The SDK checks these environment variables (in order) when `app_id` or `api_key` are not passed:

| Variable | Fallback |
|----------|----------|
| `FLAPJACK_APP_ID` | `ALGOLIA_APP_ID` |
| `FLAPJACK_API_KEY` | `ALGOLIA_API_KEY` |

Your existing `ALGOLIA_*` env vars will still work.

## 5. What doesn't change

- All method names and signatures
- All model/type names
- All search parameters
- Sync and async client patterns
- Response object structure
- Error handling patterns

## 6. Quick migration checklist

- [ ] `pip uninstall algoliasearch && pip install flapjack-search`
- [ ] Find/replace `from algoliasearch.` with `from flapjacksearch.`
- [ ] If self-hosted: configure custom hosts
- [ ] Run your test suite — everything should pass
