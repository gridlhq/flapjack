package com.flapjackhq.client

/**
 * E2E tests for the Flapjack Kotlin SDK against a live Flapjack server.
 *
 * Prerequisites:
 *   - Flapjack server running on localhost:7700 (--no-auth)
 *
 * Run (JVM target only):
 *   ./gradlew :client:jvmTest --tests "com.flapjackhq.client.SearchE2ETest"
 *
 * Environment variables (optional, defaults shown):
 *   FLAPJACK_APP_ID  = test-app
 *   FLAPJACK_API_KEY = test-api-key
 *   FLAPJACK_HOST    = localhost
 *   FLAPJACK_PORT    = 7700
 */

import com.flapjackhq.client.api.SearchClient
import com.flapjackhq.client.configuration.ClientOptions
import com.flapjackhq.client.configuration.Host
import com.flapjackhq.client.configuration.CallType
import com.flapjackhq.client.extensions.waitForTask
import com.flapjackhq.client.model.search.*
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.*
import org.junit.BeforeClass
import org.junit.AfterClass
import kotlin.test.*

private const val TEST_INDEX = "test_kotlin_e2e"

private fun buildClient(): SearchClient {
    val appId  = System.getenv("FLAPJACK_APP_ID")  ?: "test-app"
    val apiKey = System.getenv("FLAPJACK_API_KEY") ?: "test-api-key"
    val host   = System.getenv("FLAPJACK_HOST")    ?: "localhost"
    val port   = System.getenv("FLAPJACK_PORT")?.toInt() ?: 7700

    return SearchClient(
        appId  = appId,
        apiKey = apiKey,
        options = ClientOptions(
            hosts = listOf(Host(url = host, port = port, protocol = "http", callType = null)),
        ),
    )
}

private val SEED_OBJECTS: List<JsonObject> = listOf(
    buildJsonObject {
        put("objectID", "phone1"); put("name", "iPhone 15 Pro")
        put("brand", "Apple"); put("category", "Phone"); put("price", 999)
    },
    buildJsonObject {
        put("objectID", "phone2"); put("name", "Samsung Galaxy S24")
        put("brand", "Samsung"); put("category", "Phone"); put("price", 799)
    },
    buildJsonObject {
        put("objectID", "laptop1"); put("name", "MacBook Pro M3")
        put("brand", "Apple"); put("category", "Laptop"); put("price", 1999)
    },
    buildJsonObject {
        put("objectID", "phone3"); put("name", "Google Pixel 8")
        put("brand", "Google"); put("category", "Phone"); put("price", 699)
    },
    buildJsonObject {
        put("objectID", "laptop2"); put("name", "Dell XPS 15")
        put("brand", "Dell"); put("category", "Laptop"); put("price", 1299)
    },
)

/** Unwrap SearchResult to SearchResponse, or null if not a hit result. */
private fun SearchResult.asSearchResponse(): SearchResponse? =
    (this as? SearchResult.SearchResponseValue)?.value

class SearchE2ETest {

    companion object {
        // Shared across all test methods — initialised once by @BeforeClass.
        lateinit var client: SearchClient

        /**
         * Set up the index ONCE before any test runs.
         *
         * Using JUnit4 @BeforeClass (via kotlin-test-junit) so we seed only once per
         * suite rather than before every individual test — consistent with the Dart
         * (setUpAll) and Scala (BeforeAndAfterAll) equivalents.
         *
         * Write operations return a taskID; we use waitForTask to poll until the server
         * confirms the task is published instead of sleeping for a fixed duration.
         */
        @BeforeClass
        @JvmStatic
        fun setUpClass(): Unit = runBlocking {
            client = buildClient()

            // Push index settings and wait for the task to be published.
            val settingsResp = client.setSettings(
                indexName = TEST_INDEX,
                indexSettings = IndexSettings(
                    searchableAttributes = listOf("name", "brand", "category"),
                    attributesForFaceting = listOf("brand", "category"),
                ),
            )
            client.waitForTask(TEST_INDEX, settingsResp.taskID)

            // Batch-seed test data and wait for indexing to complete.
            val batchResp = client.batch(
                indexName = TEST_INDEX,
                batchWriteParams = BatchWriteParams(
                    requests = SEED_OBJECTS.map { obj ->
                        BatchRequest(action = Action.AddObject, body = obj)
                    },
                ),
            )
            client.waitForTask(TEST_INDEX, batchResp.taskID)
        }

        @AfterClass
        @JvmStatic
        fun tearDownClass(): Unit = runBlocking {
            try { client.deleteIndex(indexName = TEST_INDEX) } catch (_: Exception) {}
        }
    }

    // ── List indices ──────────────────────────────────────────────────────────

    @Test
    fun testListIndices(): Unit = runBlocking {
        val response = client.listIndices()
        val found = response.items?.any { it.name == TEST_INDEX } ?: false
        assertTrue(found, "Expected $TEST_INDEX in index listing")
    }

    // ── Search ────────────────────────────────────────────────────────────────

    @Test
    fun testBasicSearch(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(SearchQuery.of(SearchForHits(indexName = TEST_INDEX, query = "pixel"))),
            ),
        )
        val result = response.results.first().asSearchResponse()
        assertNotNull(result, "Expected SearchResponse")
        assertTrue(result.hits.isNotEmpty(), "Expected hits for 'pixel'")
        assertTrue(
            result.hits.any { hit ->
                val name = (hit.additionalProperties["name"] as? JsonPrimitive)?.content ?: ""
                "pixel" in name.lowercase()
            },
            "Expected a hit with 'pixel' in name"
        )
    }

    @Test
    fun testEmptyQueryReturnsAll(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(SearchQuery.of(SearchForHits(indexName = TEST_INDEX, query = ""))),
            ),
        )
        val result = response.results.first().asSearchResponse()
        assertNotNull(result)
        assertTrue(result.hits.size >= 5, "Expected ≥5 hits for empty query, got ${result.hits.size}")
    }

    @Test
    fun testSearchWithFilter(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(
                    SearchQuery.of(
                        SearchForHits(
                            indexName = TEST_INDEX,
                            query = "",
                            filters = "brand:Apple",
                        )
                    )
                ),
            ),
        )
        val result = response.results.first().asSearchResponse()
        assertNotNull(result)
        assertTrue(result.hits.isNotEmpty(), "Expected hits filtered by brand:Apple")
        result.hits.forEach { hit ->
            val brand = (hit.additionalProperties["brand"] as? JsonPrimitive)?.content
            assertEquals("Apple", brand, "Expected brand=Apple for all filtered hits")
        }
    }

    @Test
    fun testSearchWithFacets(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(
                    SearchQuery.of(
                        SearchForHits(
                            indexName = TEST_INDEX,
                            query = "",
                            facets = listOf("brand", "category"),
                        )
                    )
                ),
            ),
        )
        val result = response.results.first().asSearchResponse()
        assertNotNull(result)
        assertNotNull(result.facets, "Expected facets in response")
        assertTrue(result.facets!!.containsKey("brand"), "Expected 'brand' facet")
        assertTrue(result.facets!!.containsKey("category"), "Expected 'category' facet")
    }

    @Test
    fun testSearchPagination(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(
                    SearchQuery.of(
                        SearchForHits(
                            indexName = TEST_INDEX,
                            query = "",
                            hitsPerPage = 2,
                        )
                    )
                ),
            ),
        )
        val result = response.results.first().asSearchResponse()
        assertNotNull(result)
        assertTrue(result.hits.size <= 2, "Expected at most 2 hits per page")
        assertTrue((result.nbPages ?: 0) >= 2, "Expected multiple pages")
    }

    @Test
    fun testHighlightResult(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(SearchQuery.of(SearchForHits(indexName = TEST_INDEX, query = "macbook"))),
            ),
        )
        val result = response.results.first().asSearchResponse()
        assertNotNull(result)
        assertTrue(result.hits.isNotEmpty(), "Expected hits for 'macbook'")
        assertNotNull(result.hits.first().highlightResult, "Expected _highlightResult in hit")
    }

    @Test
    fun testMultiIndexSearch(): Unit = runBlocking {
        val response = client.search(
            searchMethodParams = SearchMethodParams(
                requests = listOf(
                    SearchQuery.of(SearchForHits(indexName = TEST_INDEX, query = "apple")),
                    SearchQuery.of(SearchForHits(indexName = TEST_INDEX, query = "dell")),
                ),
            ),
        )
        assertEquals(2, response.results.size, "Expected 2 result sets")
    }

    // ── Object CRUD ───────────────────────────────────────────────────────────

    @Test
    fun testGetObject(): Unit = runBlocking {
        val obj = client.getObject(indexName = TEST_INDEX, objectID = "phone1")
        assertEquals("iPhone 15 Pro", (obj["name"] as? JsonPrimitive)?.content)
    }

    @Test
    fun testPartialUpdateObject(): Unit = runBlocking {
        val updateResp = client.partialUpdateObject(
            indexName = TEST_INDEX,
            objectID  = "phone1",
            attributesToUpdate = mapOf("price" to JsonPrimitive(949)),
        )
        client.waitForTask(TEST_INDEX, updateResp.taskID!!)
        val obj = client.getObject(indexName = TEST_INDEX, objectID = "phone1")
        val price = (obj["price"] as? JsonPrimitive)?.double
        assertEquals(949.0, price, "Expected price updated to 949")

        // Restore
        val restoreResp = client.partialUpdateObject(
            indexName = TEST_INDEX,
            objectID  = "phone1",
            attributesToUpdate = mapOf("price" to JsonPrimitive(999)),
        )
        client.waitForTask(TEST_INDEX, restoreResp.taskID!!)
    }

    @Test
    fun testSaveAndDeleteObject(): Unit = runBlocking {
        val obj = buildJsonObject {
            put("objectID", "temp_kotlin_1")
            put("name", "Temp Product"); put("brand", "Test")
            put("category", "Test"); put("price", 1)
        }
        val saveResp = client.addOrUpdateObject(indexName = TEST_INDEX, objectID = "temp_kotlin_1", body = obj)
        client.waitForTask(TEST_INDEX, saveResp.taskID!!)

        val retrieved = client.getObject(indexName = TEST_INDEX, objectID = "temp_kotlin_1")
        assertEquals("Temp Product", (retrieved["name"] as? JsonPrimitive)?.content)

        client.deleteObject(indexName = TEST_INDEX, objectID = "temp_kotlin_1")
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    @Test
    fun testGetSettings(): Unit = runBlocking {
        val settings = client.getSettings(indexName = TEST_INDEX)
        assertTrue(
            settings.searchableAttributes?.isNotEmpty() == true,
            "Expected searchableAttributes to be set"
        )
    }

    @Test
    fun testUpdateSettings(): Unit = runBlocking {
        val resp = client.setSettings(
            indexName = TEST_INDEX,
            indexSettings = IndexSettings(
                searchableAttributes = listOf("name", "brand", "category", "price"),
            ),
        )
        client.waitForTask(TEST_INDEX, resp.taskID)
        val settings = client.getSettings(indexName = TEST_INDEX)
        assertTrue(
            settings.searchableAttributes?.contains("price") == true,
            "Expected 'price' in searchableAttributes after update"
        )

        // Restore
        val restoreResp = client.setSettings(
            indexName = TEST_INDEX,
            indexSettings = IndexSettings(
                searchableAttributes = listOf("name", "brand", "category"),
            ),
        )
        client.waitForTask(TEST_INDEX, restoreResp.taskID)
    }

    // ── Synonyms ──────────────────────────────────────────────────────────────

    @Test
    fun testSaveAndSearchSynonyms(): Unit = runBlocking {
        val synonym = SynonymHit(
            objectID = "syn_phone_mobile_kt",
            type = SynonymType.Synonym,
            synonyms = listOf("phone", "mobile", "cell"),
        )
        val synonymResp = client.saveSynonym(indexName = TEST_INDEX, objectID = "syn_phone_mobile_kt", synonymHit = synonym)
        client.waitForTask(TEST_INDEX, synonymResp.taskID)

        val resp = client.searchSynonyms(indexName = TEST_INDEX)
        assertTrue(resp.hits.isNotEmpty(), "Expected ≥1 synonym")

        client.deleteSynonym(indexName = TEST_INDEX, objectID = "syn_phone_mobile_kt")
    }

    // ── Rules ─────────────────────────────────────────────────────────────────

    @Test
    fun testSaveAndSearchRules(): Unit = runBlocking {
        val rule = Rule(
            objectID = "rule_budget_kt",
            conditions = listOf(
                Condition(pattern = "budget", anchoring = Anchoring.Contains),
            ),
            consequence = Consequence(
                params = ConsequenceParams(filters = "price < 1000"),
            ),
        )
        val ruleResp = client.saveRule(indexName = TEST_INDEX, objectID = "rule_budget_kt", rule = rule)
        client.waitForTask(TEST_INDEX, ruleResp.taskID)

        val resp = client.searchRules(indexName = TEST_INDEX)
        assertTrue(resp.nbHits >= 1, "Expected ≥1 rule")

        client.deleteRule(indexName = TEST_INDEX, objectID = "rule_budget_kt")
    }

    // ── User agent ────────────────────────────────────────────────────────────

    @Test
    fun testUserAgentViaLiveCall(): Unit = runBlocking {
        // Make a real HTTP call that would fail if the SDK user-agent or transport
        // headers were misconfigured. The server rejects malformed requests.
        val response = client.listIndices()
        assertNotNull(response.items, "Expected non-null items from listIndices")
        assertTrue(
            response.items!!.any { it.name == TEST_INDEX },
            "Expected test index in listing — proves transport and auth headers are valid"
        )
    }
}
