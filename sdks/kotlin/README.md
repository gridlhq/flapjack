# Flapjack Search Kotlin Client

Official Kotlin Multiplatform client for the [Flapjack Search API](https://github.com/flapjackhq).

Drop-in replacement for `algoliasearch-client-kotlin`. Algolia-compatible REST API with Flapjack-native host routing.

Supports JVM, iOS, and macOS targets via Kotlin Multiplatform.

## Installation

### Gradle (Kotlin DSL)

```kotlin
dependencies {
    implementation("com.flapjackhq:flapjack-search-kotlin:0.1.0")
}
```

### Gradle (Groovy)

```groovy
dependencies {
    implementation 'com.flapjackhq:flapjack-search-kotlin:0.1.0'
}
```

## Quick Start

```kotlin
import com.flapjackhq.client.api.SearchClient
import com.flapjackhq.client.model.search.*

val client = SearchClient(appId = "YOUR_APP_ID", apiKey = "YOUR_API_KEY")

// Search
val response = client.search(
    searchMethodParams = SearchMethodParams(
        requests = listOf(
            SearchQuery.ofSearchForHits(
                SearchForHits(query = "iPhone", indexName = "products")
            )
        )
    )
)
```

## Custom Host Configuration

```kotlin
import com.flapjackhq.client.configuration.*

val client = SearchClient(
    appId = "YOUR_APP_ID",
    apiKey = "YOUR_API_KEY",
    options = ClientOptions(
        hosts = listOf(
            Host("your-server.example.com", protocol = "https")
        )
    )
)
```

## Migrating from Algolia

1. Replace dependency: `com.algolia:algoliasearch-client-kotlin` -> `com.flapjackhq:flapjack-search-kotlin`
2. Update imports: `com.algolia.client.*` -> `com.flapjackhq.client.*`
3. Update agent: `AlgoliaAgent` -> `FlapjackAgent`
4. Update env vars: `ALGOLIA_*` -> `FLAPJACK_*`

Wire protocol is fully compatible - no server-side changes needed.

## Running Tests

```bash
./gradlew :client:jvmMainClasses  # Compile JVM target
```

## License

MIT
