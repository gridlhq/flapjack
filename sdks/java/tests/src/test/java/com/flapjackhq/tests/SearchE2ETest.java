package com.flapjackhq.tests;

import com.flapjackhq.api.SearchClient;
import com.flapjackhq.config.*;
import com.flapjackhq.model.search.*;

import org.junit.jupiter.api.*;

import java.util.*;

import static org.junit.jupiter.api.Assertions.*;

@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
public class SearchE2ETest {

    private static final String TEST_INDEX = "test_java_sdk";
    private static SearchClient client;

    @BeforeAll
    static void setup() {
        String appId = System.getenv("FLAPJACK_APP_ID");
        if (appId == null || appId.isEmpty()) appId = "test-app";

        String apiKey = System.getenv("FLAPJACK_API_KEY");
        if (apiKey == null || apiKey.isEmpty()) apiKey = "test-api-key";

        String host = System.getenv("FLAPJACK_HOST");
        if (host == null || host.isEmpty()) host = "localhost";

        String portStr = System.getenv("FLAPJACK_PORT");
        int port = (portStr != null && !portStr.isEmpty()) ? Integer.parseInt(portStr) : 7700;

        ClientOptions options = ClientOptions.builder()
            .setHosts(Collections.singletonList(
                new Host(host, EnumSet.of(CallType.READ, CallType.WRITE), "http", port)
            ))
            .build();

        client = new SearchClient(appId, apiKey, options);

        // Seed test data
        seedData();
    }

    static void seedData() {
        // Set settings
        IndexSettings settings = new IndexSettings()
            .setSearchableAttributes(Arrays.asList("name", "brand", "category"))
            .setAttributesForFaceting(Arrays.asList("brand", "category", "price"));
        client.setSettings(TEST_INDEX, settings);

        // Batch add objects
        List<BatchRequest> requests = Arrays.asList(
            new BatchRequest().setAction(Action.ADD_OBJECT).setBody(Map.of(
                "objectID", "phone1", "name", "iPhone 15 Pro", "brand", "Apple", "category", "Phone", "price", 999
            )),
            new BatchRequest().setAction(Action.ADD_OBJECT).setBody(Map.of(
                "objectID", "phone2", "name", "Samsung Galaxy S24", "brand", "Samsung", "category", "Phone", "price", 799
            )),
            new BatchRequest().setAction(Action.ADD_OBJECT).setBody(Map.of(
                "objectID", "laptop1", "name", "MacBook Pro M3", "brand", "Apple", "category", "Laptop", "price", 1999
            )),
            new BatchRequest().setAction(Action.ADD_OBJECT).setBody(Map.of(
                "objectID", "laptop2", "name", "Google Pixel 8", "brand", "Google", "category", "Phone", "price", 699
            )),
            new BatchRequest().setAction(Action.ADD_OBJECT).setBody(Map.of(
                "objectID", "laptop3", "name", "Dell XPS 15", "brand", "Dell", "category", "Laptop", "price", 1299
            ))
        );

        BatchWriteParams params = new BatchWriteParams().setRequests(requests);
        client.batch(TEST_INDEX, params);

        // Wait for indexing
        try { Thread.sleep(500); } catch (InterruptedException e) { /* ignore */ }
    }

    @AfterAll
    static void cleanup() {
        try {
            client.deleteIndex("test_java_sdk_copy");
        } catch (Exception e) { /* ignore */ }
        try {
            client.deleteIndex("test_java_sdk_moved");
        } catch (Exception e) { /* ignore */ }
    }

    // --- List Indices ---

    @Test
    @Order(1)
    void testListIndices() {
        ListIndicesResponse response = client.listIndices();
        assertNotNull(response);
        assertNotNull(response.getItems());
        boolean found = response.getItems().stream()
            .anyMatch(idx -> TEST_INDEX.equals(idx.getName()));
        assertTrue(found, "Test index should appear in listIndices");
    }

    // --- Basic Search ---

    @Test
    @Order(2)
    void testBasicSearch() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits().setIndexName(TEST_INDEX).setQuery("iPhone")
            ),
            Map.class
        );
        assertNotNull(response);
        assertNotNull(response.getResults());
        assertFalse(response.getResults().isEmpty());

        SearchResult<Map> result = response.getResults().get(0);
        assertTrue(result instanceof SearchResponse);
        SearchResponse<Map> searchResp = (SearchResponse<Map>) result;
        assertTrue(searchResp.getNbHits() >= 1, "Should find at least 1 hit for 'iPhone'");
        assertFalse(searchResp.getHits().isEmpty());
    }

    // --- Empty Query Returns All ---

    @Test
    @Order(3)
    void testEmptyQueryReturnsAll() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits().setIndexName(TEST_INDEX).setQuery("")
            ),
            Map.class
        );
        SearchResponse<Map> searchResp = (SearchResponse<Map>) response.getResults().get(0);
        assertTrue(searchResp.getNbHits() >= 5, "Empty query should return all 5 docs");
    }

    // --- Search With Filters ---

    @Test
    @Order(4)
    void testSearchWithFilters() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits()
                    .setIndexName(TEST_INDEX)
                    .setQuery("")
                    .setFilters("brand:Apple")
            ),
            Map.class
        );
        SearchResponse<Map> searchResp = (SearchResponse<Map>) response.getResults().get(0);
        assertTrue(searchResp.getNbHits() >= 1, "Should find Apple products");

        for (Map hit : searchResp.getHits()) {
            assertEquals("Apple", hit.get("brand"), "All filtered results should be Apple");
        }
    }

    // --- Search With Facets ---

    @Test
    @Order(5)
    void testSearchWithFacets() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits()
                    .setIndexName(TEST_INDEX)
                    .setQuery("")
                    .setFacets(Arrays.asList("brand", "category"))
            ),
            Map.class
        );
        SearchResponse<Map> searchResp = (SearchResponse<Map>) response.getResults().get(0);
        assertNotNull(searchResp.getFacets(), "Facets should be present");
        assertTrue(searchResp.getFacets().containsKey("brand"), "brand facet should exist");
        assertTrue(searchResp.getFacets().containsKey("category"), "category facet should exist");

        Map<String, Integer> brandFacets = searchResp.getFacets().get("brand");
        assertTrue(brandFacets.containsKey("Apple"), "Apple should be in brand facets");
    }

    // --- Search Highlighting ---

    @Test
    @Order(6)
    void testSearchHighlighting() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits()
                    .setIndexName(TEST_INDEX)
                    .setQuery("MacBook")
            ),
            Map.class
        );
        SearchResponse<Map> searchResp = (SearchResponse<Map>) response.getResults().get(0);
        assertTrue(searchResp.getNbHits() >= 1, "Should find MacBook");
        Map hit = searchResp.getHits().get(0);
        assertNotNull(hit.get("_highlightResult"), "_highlightResult should be present");

        @SuppressWarnings("unchecked")
        Map<String, Object> highlightResult = (Map<String, Object>) hit.get("_highlightResult");
        assertNotNull(highlightResult.get("name"), "name highlight should exist");

        @SuppressWarnings("unchecked")
        Map<String, Object> nameHighlight = (Map<String, Object>) highlightResult.get("name");
        String value = (String) nameHighlight.get("value");
        assertTrue(value.contains("<em>"), "Highlight should contain <em> tags");
    }

    // --- Search Pagination ---

    @Test
    @Order(7)
    void testSearchPagination() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams().addRequests(
                new SearchForHits()
                    .setIndexName(TEST_INDEX)
                    .setQuery("")
                    .setHitsPerPage(2)
                    .setPage(0)
            ),
            Map.class
        );
        SearchResponse<Map> searchResp = (SearchResponse<Map>) response.getResults().get(0);
        assertEquals(2, searchResp.getHits().size(), "Should return exactly 2 hits per page");
        assertTrue(searchResp.getNbPages() >= 2, "Should have multiple pages");
    }

    // --- Multi-Index Search ---

    @Test
    @Order(8)
    void testMultiIndexSearch() {
        SearchResponses<Map> response = client.search(
            new SearchMethodParams()
                .addRequests(new SearchForHits().setIndexName(TEST_INDEX).setQuery("iPhone"))
                .addRequests(new SearchForHits().setIndexName(TEST_INDEX).setQuery("Dell")),
            Map.class
        );
        assertEquals(2, response.getResults().size(), "Should get 2 results for 2 queries");

        SearchResponse<Map> first = (SearchResponse<Map>) response.getResults().get(0);
        SearchResponse<Map> second = (SearchResponse<Map>) response.getResults().get(1);
        assertTrue(first.getNbHits() >= 1, "First query should find iPhone");
        assertTrue(second.getNbHits() >= 1, "Second query should find Dell");
    }

    // --- Get Object ---

    @Test
    @Order(9)
    void testGetObject() {
        @SuppressWarnings("unchecked")
        Map<String, Object> obj = (Map<String, Object>) client.getObject(TEST_INDEX, "phone1");
        assertNotNull(obj);
        assertEquals("iPhone 15 Pro", obj.get("name"));
        assertEquals("Apple", obj.get("brand"));
    }

    // --- Partial Update Object ---

    @Test
    @Order(10)
    void testPartialUpdateObject() {
        Map<String, Object> update = new HashMap<>();
        update.put("price", 899);

        client.partialUpdateObject(TEST_INDEX, "phone2", update);
        try { Thread.sleep(300); } catch (InterruptedException e) { /* ignore */ }

        @SuppressWarnings("unchecked")
        Map<String, Object> obj = (Map<String, Object>) client.getObject(TEST_INDEX, "phone2");
        // price may come back as Integer or Double depending on serialization
        Number price = (Number) obj.get("price");
        assertEquals(899, price.intValue(), "Price should be updated to 899");
        assertEquals("Samsung Galaxy S24", obj.get("name"), "Name should be unchanged");
    }

    // --- Save and Delete Object ---

    @Test
    @Order(11)
    void testSaveAndDeleteObject() {
        Map<String, Object> newObj = new HashMap<>();
        newObj.put("objectID", "temp1");
        newObj.put("name", "Temporary Item");
        newObj.put("brand", "Test");

        // Use batch to save (more reliable than saveObject for indexing)
        client.batch(TEST_INDEX, new BatchWriteParams().setRequests(
            Collections.singletonList(
                new BatchRequest().setAction(Action.ADD_OBJECT).setBody(newObj)
            )
        ));
        try { Thread.sleep(1500); } catch (InterruptedException e) { /* ignore */ }

        @SuppressWarnings("unchecked")
        Map<String, Object> fetched = (Map<String, Object>) client.getObject(TEST_INDEX, "temp1");
        assertEquals("Temporary Item", fetched.get("name"));

        client.deleteObject(TEST_INDEX, "temp1");
        try { Thread.sleep(1000); } catch (InterruptedException e) { /* ignore */ }

        try {
            client.getObject(TEST_INDEX, "temp1");
            fail("Should have thrown exception for deleted object");
        } catch (Exception e) {
            // Expected - object was deleted
            assertTrue(e.getMessage().contains("404") || e.getMessage().contains("not found"),
                "Should be a 404/not found error");
        }
    }

    // --- Get Settings ---

    @Test
    @Order(12)
    void testGetSettings() {
        SettingsResponse settings = client.getSettings(TEST_INDEX);
        assertNotNull(settings);
        assertNotNull(settings.getSearchableAttributes());
        assertTrue(settings.getSearchableAttributes().contains("name"));
        assertTrue(settings.getSearchableAttributes().contains("brand"));
    }

    // --- Update Settings ---

    @Test
    @Order(13)
    void testUpdateSettings() {
        // Update searchableAttributes to a new value
        IndexSettings newSettings = new IndexSettings()
            .setSearchableAttributes(Arrays.asList("name", "brand", "category", "description"));
        client.setSettings(TEST_INDEX, newSettings);
        try { Thread.sleep(1500); } catch (InterruptedException e) { /* ignore */ }

        SettingsResponse settings = client.getSettings(TEST_INDEX);
        assertNotNull(settings.getSearchableAttributes());
        assertTrue(settings.getSearchableAttributes().contains("description"),
            "searchableAttributes should include 'description' after update");

        // Reset to original
        client.setSettings(TEST_INDEX, new IndexSettings()
            .setSearchableAttributes(Arrays.asList("name", "brand", "category")));
        try { Thread.sleep(500); } catch (InterruptedException e) { /* ignore */ }
    }

    // --- Save and Search Synonyms ---

    @Test
    @Order(14)
    void testSaveAndSearchSynonyms() {
        SynonymHit synonym = new SynonymHit()
            .setObjectID("syn-phone-mobile")
            .setType(SynonymType.SYNONYM)
            .setSynonyms(Arrays.asList("phone", "mobile", "smartphone"));

        client.saveSynonyms(TEST_INDEX, Collections.singletonList(synonym));
        try { Thread.sleep(300); } catch (InterruptedException e) { /* ignore */ }

        SearchSynonymsResponse synResponse = client.searchSynonyms(TEST_INDEX);
        assertNotNull(synResponse);
        assertTrue(synResponse.getNbHits() >= 1, "Should have at least 1 synonym");

        boolean found = synResponse.getHits().stream()
            .anyMatch(s -> "syn-phone-mobile".equals(s.getObjectID()));
        assertTrue(found, "Should find the synonym we saved");
    }

    // --- Save and Search Rules ---

    @Test
    @Order(15)
    void testSaveAndSearchRules() {
        Rule rule = new Rule()
            .setObjectID("rule-promo")
            .setConditions(Collections.singletonList(
                new Condition().setPattern("promo").setAnchoring(Anchoring.CONTAINS)
            ))
            .setConsequence(new Consequence()
                .setParams(new ConsequenceParams().setFilters("brand:Apple"))
            );

        client.saveRules(TEST_INDEX, Collections.singletonList(rule));
        try { Thread.sleep(500); } catch (InterruptedException e) { /* ignore */ }

        SearchRulesResponse rulesResponse = client.searchRules(TEST_INDEX);
        assertNotNull(rulesResponse);
        assertTrue(rulesResponse.getNbHits() >= 1, "Should have at least 1 rule");

        boolean found = rulesResponse.getHits().stream()
            .anyMatch(r -> "rule-promo".equals(r.getObjectID()));
        assertTrue(found, "Should find the rule we saved");
    }

    // --- User Agent Contains Flapjack ---

    @Test
    @Order(16)
    void testUserAgentContainsFlapjack() {
        FlapjackAgent agent = new FlapjackAgent("4.35.0");
        String agentString = agent.toString();
        assertTrue(agentString.contains("Flapjack for Java"),
            "User-Agent should contain 'Flapjack for Java', got: " + agentString);
    }
}
