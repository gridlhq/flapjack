# Migrating from Algolia to Flapjack (Ruby)

## Gem Change

```ruby
# Before (Algolia)
gem 'algolia'

# After (Flapjack)
gem 'flapjack-search'
```

## Require Change

```ruby
# Before
require 'algolia'

# After
require 'flapjack'
```

## Namespace Change

```ruby
# Before
client = Algolia::SearchClient.create(app_id, api_key)

# After
client = Flapjack::SearchClient.create(app_id, api_key)
```

## Error Classes

| Algolia | Flapjack |
|---------|----------|
| `Algolia::AlgoliaError` | `Flapjack::FlapjackError` |
| `Algolia::AlgoliaHttpError` | `Flapjack::FlapjackHttpError` |
| `Algolia::AlgoliaUnreachableHostError` | `Flapjack::FlapjackUnreachableHostError` |

## Self-Hosted Setup

For self-hosted Flapjack servers, configure custom hosts:

```ruby
require 'flapjack'

hosts = [
  Flapjack::Transport::StatefulHost.new(
    'localhost',
    protocol: 'http://',
    port: 7700,
    accept: CallType::READ | CallType::WRITE
  )
]

config = Flapjack::Configuration.new('app-id', 'api-key', hosts, 'Search')
client = Flapjack::SearchClient.create_with_config(config)
```

## What Stays the Same

- All API methods (`search`, `save_objects`, `get_object`, etc.)
- Wire protocol (`x-algolia-*` HTTP headers)
- Search parameters and response format
- Model class structure
- InstantSearch frontend compatibility
