# Flapjack Search Swift Client

Official Swift client for the [Flapjack Search API](https://github.com/flapjackhq).

Drop-in replacement for `AlgoliaSearchClient` Swift package. Algolia-compatible REST API with Flapjack-native host routing.

## Installation

### Swift Package Manager

Add to your `Package.swift`:

```swift
dependencies: [
    .package(url: "https://github.com/flapjackhq/flapjack-search-swift.git", from: "0.1.0")
]
```

### CocoaPods

```ruby
pod 'FlapjackSearchClient', '~> 0.1.0'
```

## Quick Start

```swift
import Search

let client = try SearchClient(appID: "YOUR_APP_ID", apiKey: "YOUR_API_KEY")

// Search
let response = try await client.search(
    searchMethodParams: SearchMethodParams(requests: [
        SearchQuery.searchForHits(
            SearchForHits(query: "iPhone", indexName: "products")
        )
    ])
)
```

## Custom Host Configuration

For self-hosted Flapjack instances:

```swift
let configuration = try SearchClientConfiguration(
    appID: "YOUR_APP_ID",
    apiKey: "YOUR_API_KEY",
    hosts: [Host(url: "your-server.example.com", port: 443, scheme: "https", callType: .readWrite)]
)
let client = SearchClient(configuration: configuration)
```

## Migrating from Algolia

1. Replace package: `AlgoliaSearchClient` -> `FlapjackSearchClient`
2. Update error types: `AlgoliaError` -> `FlapjackError`
3. Update retry strategy: `AlgoliaRetryStrategy` -> `FlapjackRetryStrategy`
4. Update env vars: `ALGOLIA_*` -> `FLAPJACK_*`

Wire protocol is fully compatible - no server-side changes needed.

## Supported Platforms

- iOS 14.0+
- macOS 11.0+
- tvOS 14.0+
- watchOS 7.0+
- Linux (with swift-crypto)

## License

MIT
