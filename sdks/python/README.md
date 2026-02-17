# Flapjack Search Python SDK

A fully-featured Python API client for [Flapjack Search](https://github.com/flapjackhq/flapjack-search-python). Drop-in replacement for `algoliasearch`.

[![PyPI](https://img.shields.io/pypi/v/flapjack-search.svg)](https://pypi.org/project/flapjack-search)
[![Python](https://img.shields.io/badge/python-3.8+-blue)](https://pypi.org/project/flapjack-search)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)

## Getting Started

```bash
pip install flapjack-search
```

### Basic usage

```python
from flapjacksearch.search.client import SearchClientSync
from flapjacksearch.search.config import SearchConfig
from flapjacksearch.http.hosts import Host, HostsCollection, CallType

# For self-hosted Flapjack server
config = SearchConfig("my-app", "my-api-key")
config.hosts = HostsCollection([
    Host(url="search.example.com", scheme="https", accept=CallType.READ | CallType.WRITE)
])
client = SearchClientSync.create_with_config(config=config)

# Index some data
client.save_objects(
    index_name="products",
    objects=[
        {"objectID": "1", "name": "iPhone 15", "brand": "Apple", "price": 999},
        {"objectID": "2", "name": "Galaxy S24", "brand": "Samsung", "price": 899},
    ],
)

# Search
from flapjacksearch.search.models import SearchMethodParams, SearchForHits, SearchQuery

result = client.search(
    search_method_params=SearchMethodParams(
        requests=[SearchQuery(SearchForHits(index_name="products", query="iphone"))]
    )
)

for hit in result.results[0].actual_instance.hits:
    print(hit.name, hit.price)
```

### Local development

```python
config = SearchConfig("test-app", "test-api-key")
config.hosts = HostsCollection([
    Host(url="localhost:7700", scheme="http", accept=CallType.READ | CallType.WRITE)
])
client = SearchClientSync.create_with_config(config=config)
```

### Async support

```python
from flapjacksearch.search.client import SearchClient

# Async client — same API, just use await
client = SearchClient.create_with_config(config=config)
result = await client.search(...)
```

## Migrating from Algolia?

Switching from `algoliasearch` takes about 5 minutes:

```bash
pip uninstall algoliasearch
pip install flapjack-search
```

```diff
- from algoliasearch.search.client import SearchClientSync
+ from flapjacksearch.search.client import SearchClientSync

- from algoliasearch.search.models import SearchMethodParams, SearchForHits, SearchQuery
+ from flapjacksearch.search.models import SearchMethodParams, SearchForHits, SearchQuery
```

All method signatures, parameters, and response types are identical. Your existing code works with just the import change.

See [MIGRATION.md](MIGRATION.md) for the full migration guide.

## API compatibility

Every method from Algolia's Python v4 SDK works identically:

| Operation | Method | Works? |
|-----------|--------|--------|
| Search | `client.search(...)` | Yes |
| Save objects | `client.save_objects(...)` | Yes |
| Get object | `client.get_object(...)` | Yes |
| Partial update | `client.partial_update_object(...)` | Yes |
| Delete object | `client.delete_object(...)` | Yes |
| Get settings | `client.get_settings(...)` | Yes |
| Set settings | `client.set_settings(...)` | Yes |
| Synonyms | `client.save_synonyms(...)` | Yes |
| Rules | `client.save_rules(...)` | Yes |
| List indices | `client.list_indices()` | Yes |
| Wait for task | `client.wait_for_task(...)` | Yes |
| API keys | `client.add_api_key(...)` | Yes |

## Troubleshooting

Encountering an issue? [Open a GitHub issue](https://github.com/flapjackhq/flapjack-search-python/issues/new) and we'll help.

## License

MIT
