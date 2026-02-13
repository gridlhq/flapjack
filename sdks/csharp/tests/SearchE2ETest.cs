using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Flapjack.Search.Clients;
using Flapjack.Search.Models.Search;
using Flapjack.Search.Transport;
using Xunit;

namespace Flapjack.Search.Tests;

public class SearchE2ETest : IAsyncLifetime
{
    private SearchClient _client;
    private const string TestIndex = "test_csharp_e2e";
    private const string AppId = "test-app";
    private const string ApiKey = "test-api-key";
    private const string Host = "localhost";
    private const int Port = 7700;

    public async Task InitializeAsync()
    {
        var config = new SearchConfig(AppId, ApiKey);
        config.CustomHosts = new List<StatefulHost>
        {
            new()
            {
                Url = Host,
                Port = Port,
                Scheme = HttpScheme.Http,
                Up = true,
                LastUse = DateTime.UtcNow,
                Accept = CallType.Read | CallType.Write,
            }
        };
        _client = new SearchClient(config);

        // Seed test data using batch
        var records = new List<BatchRequest>
        {
            new(Action.AddObject, new Dictionary<string, object>
            {
                {"objectID", "1"}, {"name", "iPhone 15 Pro"}, {"brand", "Apple"}, {"price", 999}, {"category", "electronics"}
            }),
            new(Action.AddObject, new Dictionary<string, object>
            {
                {"objectID", "2"}, {"name", "Samsung Galaxy S24"}, {"brand", "Samsung"}, {"price", 899}, {"category", "electronics"}
            }),
            new(Action.AddObject, new Dictionary<string, object>
            {
                {"objectID", "3"}, {"name", "Google Pixel 8"}, {"brand", "Google"}, {"price", 699}, {"category", "electronics"}
            }),
            new(Action.AddObject, new Dictionary<string, object>
            {
                {"objectID", "4"}, {"name", "MacBook Air M3"}, {"brand", "Apple"}, {"price", 1099}, {"category", "computers"}
            }),
            new(Action.AddObject, new Dictionary<string, object>
            {
                {"objectID", "5"}, {"name", "iPad Pro"}, {"brand", "Apple"}, {"price", 799}, {"category", "tablets"}
            }),
        };

        await _client.BatchAsync(TestIndex, new BatchWriteParams(records));
        await Task.Delay(1500);
    }

    public Task DisposeAsync() => Task.CompletedTask;

    [Fact]
    public async Task TestListIndices()
    {
        var response = await _client.ListIndicesAsync();
        Assert.NotNull(response);
        Assert.NotNull(response.Items);
    }

    [Fact]
    public async Task TestBasicSearch()
    {
        var searchParams = new SearchParams(new SearchParamsObject { Query = "iPhone" });
        var response = await _client.SearchSingleIndexAsync<object>(TestIndex, searchParams);
        Assert.NotNull(response);
        Assert.NotNull(response.Hits);
        Assert.True(response.Hits.Count > 0, "Expected at least one hit for 'iPhone'");
    }

    [Fact]
    public async Task TestEmptyQuery()
    {
        var searchParams = new SearchParams(new SearchParamsObject { Query = "" });
        var response = await _client.SearchSingleIndexAsync<object>(TestIndex, searchParams);
        Assert.NotNull(response);
        Assert.True(response.Hits.Count >= 5, "Expected all records returned for empty query");
    }

    [Fact]
    public async Task TestFilters()
    {
        var searchParams = new SearchParams(new SearchParamsObject
        {
            Query = "",
            Filters = "brand:Apple"
        });
        var response = await _client.SearchSingleIndexAsync<object>(TestIndex, searchParams);
        Assert.NotNull(response);
        Assert.True(response.Hits.Count >= 2, "Expected at least 2 Apple products");
    }

    [Fact]
    public async Task TestFacets()
    {
        var searchParams = new SearchParams(new SearchParamsObject
        {
            Query = "",
            Facets = new List<string> { "brand", "category" }
        });
        var response = await _client.SearchSingleIndexAsync<object>(TestIndex, searchParams);
        Assert.NotNull(response);
        Assert.NotNull(response.Facets);
        Assert.True(response.Facets.ContainsKey("brand"), "Expected 'brand' facet");
    }

    [Fact]
    public async Task TestHighlighting()
    {
        var searchParams = new SearchParams(new SearchParamsObject { Query = "iPhone" });
        var response = await _client.SearchSingleIndexAsync<Dictionary<string, object>>(TestIndex, searchParams);
        Assert.NotNull(response);
        Assert.True(response.Hits.Count > 0);
        var firstHit = response.Hits[0];
        Assert.True(firstHit.ContainsKey("_highlightResult"), "Expected highlighting in results");
    }

    [Fact]
    public async Task TestPagination()
    {
        var searchParams = new SearchParams(new SearchParamsObject
        {
            Query = "",
            HitsPerPage = 2,
            Page = 0
        });
        var response = await _client.SearchSingleIndexAsync<object>(TestIndex, searchParams);
        Assert.NotNull(response);
        Assert.True(response.Hits.Count <= 2, "Expected at most 2 hits per page");

        // Get second page
        var page2Params = new SearchParams(new SearchParamsObject
        {
            Query = "",
            HitsPerPage = 2,
            Page = 1
        });
        var response2 = await _client.SearchSingleIndexAsync<object>(TestIndex, page2Params);
        Assert.NotNull(response2);
        Assert.True(response2.Hits.Count > 0, "Expected hits on second page");
    }

    [Fact]
    public async Task TestGetObject()
    {
        var response = await _client.GetObjectAsync(TestIndex, "1");
        Assert.NotNull(response);
    }

    [Fact]
    public async Task TestPartialUpdate()
    {
        await _client.PartialUpdateObjectAsync(
            TestIndex,
            "1",
            new Dictionary<string, object> { { "price", 1099 } }
        );
        await Task.Delay(1000);

        var obj = await _client.GetObjectAsync(TestIndex, "1");
        Assert.NotNull(obj);
    }

    [Fact]
    public async Task TestSaveAndDeleteObject()
    {
        var newObj = new Dictionary<string, object>
        {
            {"objectID", "temp-csharp-100"},
            {"name", "Temporary Test Object"},
            {"brand", "TestBrand"}
        };

        // Save using batch for reliability
        await _client.BatchAsync(TestIndex, new BatchWriteParams(new List<BatchRequest>
        {
            new(Action.AddObject, newObj)
        }));
        await Task.Delay(1500);

        // Verify saved
        var saved = await _client.GetObjectAsync(TestIndex, "temp-csharp-100");
        Assert.NotNull(saved);

        // Delete
        await _client.DeleteObjectAsync(TestIndex, "temp-csharp-100");
        await Task.Delay(1500);

        // Verify deleted
        var ex = await Assert.ThrowsAsync<Exceptions.FlapjackApiException>(async () =>
        {
            await _client.GetObjectAsync(TestIndex, "temp-csharp-100");
        });
        Assert.Contains("404", ex.HttpErrorCode.ToString());
    }

    [Fact]
    public async Task TestGetSettings()
    {
        var settings = await _client.GetSettingsAsync(TestIndex);
        Assert.NotNull(settings);
    }

    [Fact]
    public async Task TestUpdateSettings()
    {
        var newSettings = new IndexSettings
        {
            SearchableAttributes = new List<string> { "name", "brand" }
        };
        await _client.SetSettingsAsync(TestIndex, newSettings);
        await Task.Delay(1500);

        var settings = await _client.GetSettingsAsync(TestIndex);
        Assert.NotNull(settings);
        Assert.NotNull(settings.SearchableAttributes);
        Assert.Contains("name", settings.SearchableAttributes);
        Assert.Contains("brand", settings.SearchableAttributes);
    }

    [Fact]
    public async Task TestSynonyms()
    {
        var synonym = new SynonymHit("syn-phone", SynonymType.Synonym)
        {
            Synonyms = new List<string> { "phone", "mobile", "cell" }
        };
        await _client.SaveSynonymAsync(TestIndex, "syn-phone", synonym);
        await Task.Delay(1000);

        var saved = await _client.GetSynonymAsync(TestIndex, "syn-phone");
        Assert.NotNull(saved);
        Assert.Equal("syn-phone", saved.ObjectID);
    }

    [Fact]
    public async Task TestRules()
    {
        var rule = new Rule("rule-promo", new Consequence
        {
            Params = new ConsequenceParams { Filters = "brand:Apple" }
        })
        {
            Conditions = new List<Condition>
            {
                new() { Pattern = "promo", Anchoring = Anchoring.Contains }
            }
        };
        await _client.SaveRuleAsync(TestIndex, "rule-promo", rule);
        await Task.Delay(1000);

        var saved = await _client.GetRuleAsync(TestIndex, "rule-promo");
        Assert.NotNull(saved);
        Assert.Equal("rule-promo", saved.ObjectID);
    }

    [Fact]
    public async Task TestUserAgent()
    {
        // Verify client was initialized — user agent is set internally
        var searchParams = new SearchParams(new SearchParamsObject { Query = "test" });
        var response = await _client.SearchSingleIndexAsync<object>(TestIndex, searchParams);
        Assert.NotNull(response);
    }

    [Fact]
    public async Task TestMultiIndex()
    {
        // Seed a second index
        var secondIndex = "test_csharp_e2e_multi";
        await _client.BatchAsync(secondIndex, new BatchWriteParams(new List<BatchRequest>
        {
            new(Action.AddObject, new Dictionary<string, object>
            {
                {"objectID", "m1"}, {"title", "Multi Index Test"}
            })
        }));
        await Task.Delay(1500);

        // Search both indices
        var result1 = await _client.SearchSingleIndexAsync<object>(TestIndex, new SearchParams(new SearchParamsObject { Query = "" }));
        var result2 = await _client.SearchSingleIndexAsync<object>(secondIndex, new SearchParams(new SearchParamsObject { Query = "" }));
        Assert.NotNull(result1);
        Assert.NotNull(result2);
        Assert.True(result1.Hits.Count > 0, "Expected hits from first index");
        Assert.True(result2.Hits.Count > 0, "Expected hits from second index");
    }
}
