# Flapjack Search Go SDK

Go client for the [Flapjack Search API](https://flapjack.io) — a drop-in replacement for `algoliasearch-client-go`.

## Installation

```bash
go get github.com/flapjackhq/flapjack-search-go/v4
```

## Quick Start

```go
package main

import (
    "fmt"
    "github.com/flapjackhq/flapjack-search-go/v4/flapjack/search"
)

func main() {
    client, _ := search.NewClient("YOUR_APP_ID", "YOUR_API_KEY")

    // Search
    resp, _ := client.Search(client.NewApiSearchRequest(
        search.NewSearchMethodParams([]search.SearchQuery{
            *search.SearchForHitsAsSearchQuery(
                search.NewSearchForHits("your_index", search.WithSearchForHitsQuery("search terms")),
            ),
        }),
    ))

    fmt.Printf("Found %d hits\n", len(resp.Results[0].SearchResponse.Hits))
}
```

## Custom Host (Self-Hosted)

```go
import (
    "github.com/flapjackhq/flapjack-search-go/v4/flapjack/call"
    "github.com/flapjackhq/flapjack-search-go/v4/flapjack/search"
    "github.com/flapjackhq/flapjack-search-go/v4/flapjack/transport"
)

client, _ := search.NewClientWithConfig(search.SearchConfiguration{
    Configuration: transport.Configuration{
        AppID:  "your-app-id",
        ApiKey: "your-api-key",
        Hosts: []transport.StatefulHost{
            transport.NewStatefulHost("http", "localhost:7700", call.IsReadWrite),
        },
    },
})
```

## Migrating from Algolia

Replace your import:
```diff
- "github.com/algolia/algoliasearch-client-go/v4/algolia/search"
+ "github.com/flapjackhq/flapjack-search-go/v4/flapjack/search"
```

The API is identical — all methods, types, and options work the same way.

## License

MIT
