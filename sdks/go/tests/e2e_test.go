package tests

import (
	"os"
	"strings"
	"testing"
	"time"

	"github.com/flapjackhq/flapjack-search-go/v4/flapjack/call"
	"github.com/flapjackhq/flapjack-search-go/v4/flapjack/search"
	"github.com/flapjackhq/flapjack-search-go/v4/flapjack/transport"
)

const testIndex = "test_go_sdk"

func getClient(t *testing.T) *search.APIClient {
	t.Helper()

	appID := os.Getenv("FLAPJACK_APP_ID")
	if appID == "" {
		appID = "test-app"
	}
	apiKey := os.Getenv("FLAPJACK_API_KEY")
	if apiKey == "" {
		apiKey = "test-api-key"
	}

	host := os.Getenv("FLAPJACK_HOST")
	if host == "" {
		host = "localhost:7700"
	}

	client, err := search.NewClientWithConfig(search.SearchConfiguration{
		Configuration: transport.Configuration{
			AppID:  appID,
			ApiKey: apiKey,
			Hosts: []transport.StatefulHost{
				transport.NewStatefulHost("http", host, call.IsReadWrite),
			},
			DefaultHeader:  make(map[string]string),
			ReadTimeout:    10 * time.Second,
			WriteTimeout:   30 * time.Second,
			ConnectTimeout: 5 * time.Second,
		},
	})
	if err != nil {
		t.Fatalf("failed to create client: %v", err)
	}

	return client
}

func seedTestData(t *testing.T, client *search.APIClient) {
	t.Helper()

	settings := search.NewIndexSettings(
		search.WithIndexSettingsSearchableAttributes([]string{"name", "brand", "category"}),
		search.WithIndexSettingsAttributesForFaceting([]string{"brand", "category", "price"}),
	)
	settingsReq := client.NewApiSetSettingsRequest(testIndex, settings)
	_, err := client.SetSettings(settingsReq)
	if err != nil {
		t.Fatalf("failed to set settings: %v", err)
	}

	objects := []search.BatchRequest{
		{Action: search.ACTION_ADD_OBJECT, Body: map[string]any{"objectID": "phone1", "name": "iPhone 15 Pro", "brand": "Apple", "category": "Phone", "price": 999}},
		{Action: search.ACTION_ADD_OBJECT, Body: map[string]any{"objectID": "phone2", "name": "Samsung Galaxy S24", "brand": "Samsung", "category": "Phone", "price": 799}},
		{Action: search.ACTION_ADD_OBJECT, Body: map[string]any{"objectID": "laptop1", "name": "MacBook Pro M3", "brand": "Apple", "category": "Laptop", "price": 1999}},
		{Action: search.ACTION_ADD_OBJECT, Body: map[string]any{"objectID": "laptop2", "name": "Google Pixel 8", "brand": "Google", "category": "Phone", "price": 699}},
		{Action: search.ACTION_ADD_OBJECT, Body: map[string]any{"objectID": "laptop3", "name": "Dell XPS 15", "brand": "Dell", "category": "Laptop", "price": 1299}},
	}

	batchReq := client.NewApiBatchRequest(testIndex, search.NewBatchWriteParams(objects))
	_, err = client.Batch(batchReq)
	if err != nil {
		t.Fatalf("failed to batch save objects: %v", err)
	}

	time.Sleep(500 * time.Millisecond)
}

func setupSuite(t *testing.T) *search.APIClient {
	t.Helper()
	client := getClient(t)
	seedTestData(t, client)
	return client
}

func cleanupSuite(t *testing.T, client *search.APIClient) {
	t.Helper()
	delReq := client.NewApiDeleteIndexRequest(testIndex)
	_, _ = client.DeleteIndex(delReq)
}

func searchQuery(indexName, query string, opts ...search.SearchForHitsOption) search.SearchQuery {
	allOpts := append([]search.SearchForHitsOption{search.WithSearchForHitsQuery(query)}, opts...)
	return *search.SearchForHitsAsSearchQuery(search.NewSearchForHits(indexName, allOpts...))
}

func strPtr(s string) *string {
	return &s
}

// =========================================================================
// List Indices
// =========================================================================

func TestListIndices(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	req := client.NewApiListIndicesRequest()
	resp, err := client.ListIndices(req)
	if err != nil {
		t.Fatalf("ListIndices failed: %v", err)
	}

	found := false
	for _, idx := range resp.Items {
		if idx.Name == testIndex {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("expected to find index %q in listing", testIndex)
	}
}

// =========================================================================
// Search Tests
// =========================================================================

func TestBasicSearch(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{searchQuery(testIndex, "pixel")},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	if len(resp.Results) == 0 {
		t.Fatal("expected at least 1 result set")
	}

	sr := resp.Results[0].SearchResponse
	if sr == nil {
		t.Fatal("expected SearchResponse in result")
	}

	if len(sr.Hits) == 0 {
		t.Fatal("expected at least 1 hit for 'pixel'")
	}

	found := false
	for _, hit := range sr.Hits {
		if name, ok := hit.AdditionalProperties["name"].(string); ok {
			if strings.Contains(strings.ToLower(name), "pixel") {
				found = true
				break
			}
		}
	}
	if !found {
		t.Error("expected a hit containing 'pixel'")
	}
}

func TestEmptyQueryReturnsAll(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{searchQuery(testIndex, "")},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	hits := resp.Results[0].SearchResponse.Hits
	if len(hits) < 5 {
		t.Errorf("expected at least 5 hits for empty query, got %d", len(hits))
	}
}

func TestSearchWithFilters(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{searchQuery(testIndex, "", search.WithSearchForHitsFilters("brand:Apple"))},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	hits := resp.Results[0].SearchResponse.Hits
	if len(hits) == 0 {
		t.Fatal("expected hits for filter brand:Apple")
	}

	for _, hit := range hits {
		brand, ok := hit.AdditionalProperties["brand"].(string)
		if !ok || brand != "Apple" {
			t.Errorf("expected brand=Apple, got %v", hit.AdditionalProperties["brand"])
		}
	}
}

func TestSearchWithFacets(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{searchQuery(testIndex, "", search.WithSearchForHitsFacets([]string{"brand", "category"}))},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	result := resp.Results[0].SearchResponse
	if result.Facets == nil {
		t.Fatal("expected facets in response")
	}

	facetMap := *result.Facets
	if _, ok := facetMap["brand"]; !ok {
		t.Error("expected 'brand' facet")
	}
	if _, ok := facetMap["category"]; !ok {
		t.Error("expected 'category' facet")
	}
}

func TestSearchHighlighting(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{searchQuery(testIndex, "macbook")},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	hits := resp.Results[0].SearchResponse.Hits
	if len(hits) == 0 {
		t.Fatal("expected hits for 'macbook'")
	}

	if hits[0].HighlightResult == nil {
		t.Fatal("expected _highlightResult in first hit")
	}
}

func TestSearchPagination(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{searchQuery(testIndex, "", search.WithSearchForHitsHitsPerPage(2))},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	result := resp.Results[0].SearchResponse
	if len(result.Hits) > 2 {
		t.Errorf("expected at most 2 hits, got %d", len(result.Hits))
	}
	if *result.NbPages <= 1 {
		t.Errorf("expected more than 1 page, got %d", *result.NbPages)
	}
}

func TestMultiIndexSearch(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	searchReq := client.NewApiSearchRequest(search.NewSearchMethodParams(
		[]search.SearchQuery{
			searchQuery(testIndex, "apple"),
			searchQuery(testIndex, "dell"),
		},
	))

	resp, err := client.Search(searchReq)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}

	if len(resp.Results) != 2 {
		t.Errorf("expected 2 result sets, got %d", len(resp.Results))
	}
}

// =========================================================================
// Object Tests
// =========================================================================

func TestGetObject(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	req := client.NewApiGetObjectRequest(testIndex, "phone1")
	resp, err := client.GetObject(req)
	if err != nil {
		t.Fatalf("GetObject failed: %v", err)
	}

	obj := *resp
	if obj["objectID"] != "phone1" {
		t.Errorf("expected objectID=phone1, got %v", obj["objectID"])
	}
	if obj["name"] != "iPhone 15 Pro" {
		t.Errorf("expected name='iPhone 15 Pro', got %v", obj["name"])
	}
}

func TestPartialUpdateObject(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	updateReq := client.NewApiPartialUpdateObjectRequest(testIndex, "phone1", map[string]any{"price": 949})
	_, err := client.PartialUpdateObject(updateReq)
	if err != nil {
		t.Fatalf("PartialUpdateObject failed: %v", err)
	}

	time.Sleep(300 * time.Millisecond)

	getReq := client.NewApiGetObjectRequest(testIndex, "phone1")
	resp, err := client.GetObject(getReq)
	if err != nil {
		t.Fatalf("GetObject failed: %v", err)
	}

	price, _ := (*resp)["price"].(float64)
	if price != 949 {
		t.Errorf("expected price=949, got %v", (*resp)["price"])
	}

	// Restore
	restoreReq := client.NewApiPartialUpdateObjectRequest(testIndex, "phone1", map[string]any{"price": 999})
	_, _ = client.PartialUpdateObject(restoreReq)
	time.Sleep(300 * time.Millisecond)
}

func TestSaveAndDeleteObject(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	saveReq := client.NewApiAddOrUpdateObjectRequest(testIndex, "temp_go_1", map[string]any{
		"name": "Temp Product", "brand": "Test", "category": "Test", "price": 1,
	})
	_, err := client.AddOrUpdateObject(saveReq)
	if err != nil {
		t.Fatalf("AddOrUpdateObject failed: %v", err)
	}

	time.Sleep(300 * time.Millisecond)

	getReq := client.NewApiGetObjectRequest(testIndex, "temp_go_1")
	resp, err := client.GetObject(getReq)
	if err != nil {
		t.Fatalf("GetObject failed: %v", err)
	}
	if (*resp)["name"] != "Temp Product" {
		t.Errorf("expected name='Temp Product', got %v", (*resp)["name"])
	}

	delReq := client.NewApiDeleteObjectRequest(testIndex, "temp_go_1")
	_, err = client.DeleteObject(delReq)
	if err != nil {
		t.Fatalf("DeleteObject failed: %v", err)
	}
}

// =========================================================================
// Settings Tests
// =========================================================================

func TestGetSettings(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	req := client.NewApiGetSettingsRequest(testIndex)
	resp, err := client.GetSettings(req)
	if err != nil {
		t.Fatalf("GetSettings failed: %v", err)
	}

	if len(resp.SearchableAttributes) == 0 {
		t.Error("expected searchableAttributes to be set")
	}
}

func TestUpdateSettings(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	newSettings := search.NewIndexSettings(
		search.WithIndexSettingsSearchableAttributes([]string{"name", "brand", "category", "price"}),
	)
	setReq := client.NewApiSetSettingsRequest(testIndex, newSettings)
	_, err := client.SetSettings(setReq)
	if err != nil {
		t.Fatalf("SetSettings failed: %v", err)
	}

	time.Sleep(300 * time.Millisecond)

	getReq := client.NewApiGetSettingsRequest(testIndex)
	resp, err := client.GetSettings(getReq)
	if err != nil {
		t.Fatalf("GetSettings failed: %v", err)
	}

	found := false
	for _, attr := range resp.SearchableAttributes {
		if attr == "price" {
			found = true
			break
		}
	}
	if !found {
		t.Error("expected 'price' in searchableAttributes after update")
	}

	// Restore
	restoreSettings := search.NewIndexSettings(
		search.WithIndexSettingsSearchableAttributes([]string{"name", "brand", "category"}),
	)
	restoreReq := client.NewApiSetSettingsRequest(testIndex, restoreSettings)
	_, _ = client.SetSettings(restoreReq)
	time.Sleep(300 * time.Millisecond)
}

// =========================================================================
// Synonyms Tests
// =========================================================================

func TestSaveAndSearchSynonyms(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	synonym := search.NewSynonymHit("syn_phone_mobile_go", search.SYNONYM_TYPE_SYNONYM,
		search.WithSynonymHitSynonyms([]string{"phone", "mobile", "cell"}),
	)

	saveReq := client.NewApiSaveSynonymRequest(testIndex, "syn_phone_mobile_go", synonym)
	_, err := client.SaveSynonym(saveReq)
	if err != nil {
		t.Fatalf("SaveSynonym failed: %v", err)
	}

	time.Sleep(300 * time.Millisecond)

	searchReq := client.NewApiSearchSynonymsRequest(testIndex)
	resp, err := client.SearchSynonyms(searchReq)
	if err != nil {
		t.Fatalf("SearchSynonyms failed: %v", err)
	}

	if len(resp.Hits) == 0 {
		t.Error("expected at least 1 synonym hit")
	}

	delReq := client.NewApiDeleteSynonymRequest(testIndex, "syn_phone_mobile_go")
	_, _ = client.DeleteSynonym(delReq)
}

// =========================================================================
// Rules Tests
// =========================================================================

func TestSaveAndSearchRules(t *testing.T) {
	client := setupSuite(t)
	defer cleanupSuite(t, client)

	rule := search.NewRule("rule_budget_go",
		*search.NewConsequence(
			search.WithConsequenceParams(
				*search.NewConsequenceParams(
					search.WithConsequenceParamsFilters("price < 1000"),
				),
			),
		),
		search.WithRuleConditions([]search.Condition{
			*search.NewCondition(
				search.WithConditionPattern("budget"),
				search.WithConditionAnchoring(search.ANCHORING_CONTAINS),
			),
		}),
	)

	saveReq := client.NewApiSaveRuleRequest(testIndex, "rule_budget_go", rule)
	_, err := client.SaveRule(saveReq)
	if err != nil {
		t.Fatalf("SaveRule failed: %v", err)
	}

	time.Sleep(300 * time.Millisecond)

	searchReq := client.NewApiSearchRulesRequest(testIndex)
	resp, err := client.SearchRules(searchReq)
	if err != nil {
		t.Fatalf("SearchRules failed: %v", err)
	}

	if resp.NbHits < 1 {
		t.Error("expected at least 1 rule hit")
	}

	delReq := client.NewApiDeleteRuleRequest(testIndex, "rule_budget_go")
	_, _ = client.DeleteRule(delReq)
}

// =========================================================================
// User Agent Tests
// =========================================================================

func TestUserAgentContainsFlapjack(t *testing.T) {
	client := getClient(t)
	cfg := client.GetConfiguration()
	if !strings.Contains(cfg.UserAgent, "Flapjack for Go") {
		t.Errorf("expected user agent to contain 'Flapjack for Go', got %q", cfg.UserAgent)
	}
}
