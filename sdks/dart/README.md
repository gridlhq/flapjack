# Flapjack Search Dart Client

Official Dart/Flutter client for the [Flapjack Search API](https://github.com/flapjackhq).

Drop-in replacement for `algoliasearch` Dart package. Algolia-compatible REST API with Flapjack-native host routing.

## Packages

| Package | Description |
|---------|-------------|
| `flapjacksearch` | Umbrella package (search + insights) |
| `flapjack_client_core` | Core HTTP, retry strategy, exceptions |
| `flapjack_client_search` | Search API client |
| `flapjack_client_insights` | Insights/events API |
| `flapjack_client_recommend` | Recommendations API |
| `flapjack_client_composition` | Composition API |
| `flapjack_client_abtesting_v3` | A/B Testing API |

## Installation

```yaml
dependencies:
  flapjacksearch: ^1.44.0
```

## Quick Start

```dart
import 'package:flapjacksearch/flapjacksearch.dart';

final client = SearchClient(appId: 'YOUR_APP_ID', apiKey: 'YOUR_API_KEY');

// Search
final response = await client.searchSingleIndex(
  indexName: 'products',
  searchParams: SearchParamsObject(query: 'iPhone'),
);
```

## Custom Host Configuration

```dart
import 'package:flapjack_client_core/flapjack_client_core.dart';

final client = SearchClient(
  appId: 'YOUR_APP_ID',
  apiKey: 'YOUR_API_KEY',
  options: ClientOptions(
    hosts: [Host(url: 'your-server.example.com', scheme: 'https')],
  ),
);
```

## Migrating from Algolia

1. Replace dependency: `algoliasearch` -> `flapjacksearch`
2. Update imports: `algolia_client_*` -> `flapjack_client_*`
3. Update env vars: `ALGOLIA_*` -> `FLAPJACK_*`

Wire protocol is fully compatible - no server-side changes needed.

## Development

```bash
# Get dependencies for a package
cd packages/client_search && dart pub get

# Analyze
dart analyze
```

## License

MIT
