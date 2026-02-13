# Flapjack Search Ruby SDK

A fully-featured Ruby API client for [Flapjack Search](https://github.com/flapjackhq/flapjack). Drop-in replacement for the `algolia` gem.

## Installation

Add to your Gemfile:

```ruby
gem 'flapjack-search', '~> 0.1.0.pre.beta.1'
```

Or install directly:

```bash
gem install flapjack-search --pre
```

## Quick Start

```ruby
require 'flapjack'

# Cloud usage (same API as Algolia)
client = Flapjack::SearchClient.create('YOUR_APP_ID', 'YOUR_API_KEY')

# Self-hosted Flapjack server
hosts = [
  Flapjack::Transport::StatefulHost.new(
    'localhost',
    protocol: 'http://',
    port: 7700,
    accept: CallType::READ | CallType::WRITE
  )
]
config = Flapjack::Configuration.new('your-app-id', 'your-api-key', hosts, 'Search')
client = Flapjack::SearchClient.create_with_config(config)

# Index documents
client.save_objects('movies', [
  { objectID: '1', title: 'The Matrix', year: 1999, genre: 'sci-fi' },
  { objectID: '2', title: 'Inception', year: 2010, genre: 'sci-fi' },
])

# Search
results = client.search(
  Flapjack::Search::SearchMethodParams.new(
    requests: [
      Flapjack::Search::SearchForHits.new(index_name: 'movies', query: 'matrix')
    ]
  )
)

puts results.results[0].hits[0].additional_properties['title']
# => "The Matrix"
```

## Features

- Full Algolia Search API compatibility
- Self-hosted Flapjack server support via custom hosts
- Typed model objects with OpenAPI-generated classes
- Automatic retry with host failover
- Synonym and query rule management
- Faceted search and filtering
- Browse/cursor-based pagination

## Migrating from Algolia

See [MIGRATION.md](MIGRATION.md) for a step-by-step guide.

Key changes:
- `gem 'algolia'` becomes `gem 'flapjack-search'`
- `require 'algolia'` becomes `require 'flapjack'`
- `Algolia::SearchClient` becomes `Flapjack::SearchClient`
- `AlgoliaError` becomes `FlapjackError`
- Wire protocol (`x-algolia-*` headers) unchanged for compatibility

## Requirements

- Ruby >= 2.6
- Flapjack server (self-hosted) or Flapjack Cloud account

## License

MIT