# Flapjack Search Java Client

Official Java client for the [Flapjack Search API](https://github.com/flapjackhq).

Drop-in replacement for `algoliasearch` Java client. Algolia-compatible REST API with Flapjack-native host routing.

## Installation

### Gradle

```groovy
dependencies {
    implementation 'com.flapjackhq:flapjacksearch:0.1.0'
}
```

### Maven

```xml
<dependency>
    <groupId>com.flapjackhq</groupId>
    <artifactId>flapjacksearch</artifactId>
    <version>0.1.0</version>
</dependency>
```

## Quick Start

```java
import com.flapjackhq.api.SearchClient;
import com.flapjackhq.model.search.*;

import java.util.*;

public class Example {
    public static void main(String[] args) {
        SearchClient client = new SearchClient("YOUR_APP_ID", "YOUR_API_KEY");

        // Index a document
        Map<String, Object> record = Map.of(
            "objectID", "1",
            "name", "iPhone 15 Pro",
            "brand", "Apple",
            "price", 999
        );
        client.saveObject("products", record);

        // Search
        SearchResponses<Map> results = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits().setIndexName("products").setQuery("iPhone")
            ),
            Map.class
        );

        SearchResponse<Map> response = (SearchResponse<Map>) results.getResults().get(0);
        System.out.println("Found " + response.getNbHits() + " hits");
    }
}
```

## Custom Host Configuration

For self-hosted Flapjack instances:

```java
import com.flapjackhq.config.*;
import java.util.*;

ClientOptions options = ClientOptions.builder()
    .setHosts(Collections.singletonList(
        new Host("your-server.example.com", EnumSet.of(CallType.READ, CallType.WRITE), "https", 443)
    ))
    .build();

SearchClient client = new SearchClient("YOUR_APP_ID", "YOUR_API_KEY", options);
```

## Migrating from Algolia

1. Replace dependency: `com.algolia:algoliasearch` -> `com.flapjackhq:flapjacksearch`
2. Update imports: `com.algolia.*` -> `com.flapjackhq.*`
3. Rename exceptions: `AlgoliaApiException` -> `FlapjackApiException`
4. Rename agent: `AlgoliaAgent` -> `FlapjackAgent`
5. Update env vars: `ALGOLIA_*` -> `FLAPJACK_*`

Wire protocol is fully compatible - no server-side changes needed.

## Running Tests

Requires a Flapjack server running on `localhost:7700`:

```bash
# Set JAVA_HOME if needed
export JAVA_HOME=/path/to/jdk

# Run E2E tests
./gradlew :tests:test
```

## License

MIT
