# Flapjack Search Scala Client

Official Scala client for the [Flapjack Search API](https://github.com/flapjackhq).

Drop-in replacement for `algoliasearch-client-scala`. Algolia-compatible REST API with Flapjack-native host routing.

Supports Scala 2.13 and 3.x.

## Installation

### SBT

```scala
libraryDependencies += "com.flapjackhq" %% "flapjacksearch-scala" % "0.1.0"
```

## Quick Start

```scala
import flapjacksearch.api.SearchClient
import flapjacksearch.search._

val client = SearchClient(appId = "YOUR_APP_ID", apiKey = "YOUR_API_KEY")

// Search
val response = client.searchSingleIndex[Any](
  indexName = "products",
  searchParams = Some(SearchParamsObject(query = Some("iPhone")))
)
```

## Custom Host Configuration

```scala
import flapjacksearch.config._

val client = SearchClient(
  appId = "YOUR_APP_ID",
  apiKey = "YOUR_API_KEY",
  clientOptions = ClientOptions(
    hosts = Some(Seq(Host("your-server.example.com", scheme = "https")))
  )
)
```

## Migrating from Algolia

1. Replace dependency: `algoliasearch-client-scala` -> `flapjacksearch-scala`
2. Update imports: `algoliasearch.*` -> `flapjacksearch.*`
3. Update env vars: `ALGOLIA_*` -> `FLAPJACK_*`

Wire protocol is fully compatible - no server-side changes needed.

## Building

```bash
sbt compile
```

## License

MIT
