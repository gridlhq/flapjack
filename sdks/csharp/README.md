# Flapjack Search C# Client

Official C#/.NET client for the [Flapjack Search API](https://github.com/flapjackhq).

Drop-in replacement for `Algolia.Search` NuGet package. Algolia-compatible REST API with Flapjack-native host routing.

Supports .NET Standard 2.0 and 2.1 (cross-platform).

## Installation

### NuGet

```bash
dotnet add package Flapjack.Search
```

### Package Manager

```powershell
Install-Package Flapjack.Search
```

## Quick Start

```csharp
using Flapjack.Search.Clients;
using Flapjack.Search.Models.Search;

var client = new SearchClient("YOUR_APP_ID", "YOUR_API_KEY");

// Search
var response = await client.SearchSingleIndexAsync<object>(
    "products",
    new SearchParams(new SearchParamsObject { Query = "iPhone" })
);
```

## Custom Host Configuration

```csharp
using Flapjack.Search.Clients;
using Flapjack.Search.Transport;

var config = new SearchConfig("YOUR_APP_ID", "YOUR_API_KEY");
config.CustomHosts = new List<StatefulHost>
{
    new()
    {
        Url = "your-server.example.com",
        Scheme = HttpScheme.Https,
        Up = true,
        Accept = CallType.Read | CallType.Write,
    }
};
var client = new SearchClient(config);
```

## Migrating from Algolia

1. Replace NuGet package: `Algolia.Search` -> `Flapjack.Search`
2. Update namespaces: `Algolia.Search.*` -> `Flapjack.Search.*`
3. Update config class: `AlgoliaConfig` -> `FlapjackConfig`
4. Update exceptions: `AlgoliaException` -> `FlapjackException`
5. Update env vars: `ALGOLIA_*` -> `FLAPJACK_*`

Wire protocol is fully compatible - no server-side changes needed.

## Running Tests

```bash
dotnet build Flapjack.Search.sln
dotnet test tests/Flapjack.Search.Tests.csproj
```

## License

MIT