package flapjacksearch

/**
 * E2E tests for the Flapjack Scala SDK against a live Flapjack server.
 *
 * Prerequisites:
 *   - Flapjack server running on localhost:7700 (--no-auth)
 *
 * Run:
 *   sbt test
 *
 * Environment variables (optional, defaults shown):
 *   FLAPJACK_APP_ID  = test-app
 *   FLAPJACK_API_KEY = test-api-key
 *   FLAPJACK_HOST    = localhost
 *   FLAPJACK_PORT    = 7700
 */

import flapjacksearch.api.SearchClient
import flapjacksearch.config.{CallType, ClientOptions, Host}
import flapjacksearch.extension._
import flapjacksearch.search._

import org.json4s._
import org.json4s.native.JsonMethods._
import org.scalatest.BeforeAndAfterAll
import org.scalatest.funsuite.AnyFunSuite
import org.scalatest.matchers.should.Matchers

import scala.concurrent.{Await, ExecutionContext, Future}
import scala.concurrent.duration._

class SearchE2ETest extends AnyFunSuite with Matchers with BeforeAndAfterAll {

  implicit val ec: ExecutionContext = ExecutionContext.global
  implicit val formats: Formats = DefaultFormats

  private val testIndex = "test_scala_e2e"

  private val appId  = sys.env.getOrElse("FLAPJACK_APP_ID",  "test-app")
  private val apiKey = sys.env.getOrElse("FLAPJACK_API_KEY", "test-api-key")
  private val host   = sys.env.getOrElse("FLAPJACK_HOST",    "localhost")
  private val port   = sys.env.getOrElse("FLAPJACK_PORT",    "7700").toInt

  private lazy val client: SearchClient = SearchClient(
    appId  = appId,
    apiKey = apiKey,
    clientOptions = ClientOptions(
      hosts = Seq(Host(url = host, callTypes = Set(CallType.Read, CallType.Write), scheme = "http", port = Some(port))),
    )
  )

  private val seedObjects = Seq(
    Map("objectID" -> "phone1",  "name" -> "iPhone 15 Pro",     "brand" -> "Apple",   "category" -> "Phone",  "price" -> 999),
    Map("objectID" -> "phone2",  "name" -> "Samsung Galaxy S24", "brand" -> "Samsung", "category" -> "Phone",  "price" -> 799),
    Map("objectID" -> "laptop1", "name" -> "MacBook Pro M3",     "brand" -> "Apple",   "category" -> "Laptop", "price" -> 1999),
    Map("objectID" -> "phone3",  "name" -> "Google Pixel 8",     "brand" -> "Google",  "category" -> "Phone",  "price" -> 699),
    Map("objectID" -> "laptop2", "name" -> "Dell XPS 15",        "brand" -> "Dell",    "category" -> "Laptop", "price" -> 1299),
  )

  private def await[T](f: Future[T], timeout: FiniteDuration = 30.seconds): T =
    Await.result(f, timeout)

  /** Unwrap a SearchResult to SearchResponse, failing the test if it's the wrong type. */
  private def asSearchResponse(r: SearchResult): SearchResponse = r match {
    case sr: SearchResponse => sr
    case other => fail(s"Expected SearchResponse but got ${other.getClass.getSimpleName}")
  }

  override def beforeAll(): Unit = {
    // Push settings and wait for the task to be published before seeding.
    val settingsResp = await(client.setSettings(
      indexName = testIndex,
      indexSettings = IndexSettings(
        searchableAttributes = Some(Seq("name", "brand", "category")),
        attributesForFaceting = Some(Seq("brand", "category")),
      )
    ))
    await(client.waitForTask(testIndex, settingsResp.taskID))

    // Batch seed and wait for indexing to complete before any test runs.
    val batchRequests = seedObjects.map(obj =>
      BatchRequest(action = Action.AddObject, body = obj)
    )
    val batchResp = await(client.batch(
      indexName = testIndex,
      batchWriteParams = BatchWriteParams(requests = batchRequests)
    ))
    await(client.waitForTask(testIndex, batchResp.taskID))
  }

  override def afterAll(): Unit = {
    try { await(client.deleteIndex(indexName = testIndex)) } catch { case _: Exception => }
  }

  // ── List indices ────────────────────────────────────────────────────────────

  test("listIndices contains test index") {
    val resp = await(client.listIndices())
    resp.items should not be empty
    resp.items.exists(_.name == testIndex) shouldBe true
  }

  // ── Search ──────────────────────────────────────────────────────────────────

  test("basic text search returns relevant hit") {
    val resp = await(client.search(
      searchMethodParams = SearchMethodParams(requests = Seq(
        SearchForHits(indexName = testIndex, query = Some("pixel"))
      ))
    ))
    val result = asSearchResponse(resp.results.head)
    result.hits should not be empty
    result.hits.exists { hit =>
      hit.get("name").exists(_.toString.toLowerCase.contains("pixel"))
    } shouldBe true
  }

  test("empty query returns all documents") {
    val resp = await(client.search(
      searchMethodParams = SearchMethodParams(requests = Seq(
        SearchForHits(indexName = testIndex, query = Some(""))
      ))
    ))
    asSearchResponse(resp.results.head).hits.size should be >= 5
  }

  test("search with filter by brand") {
    val resp = await(client.search(
      searchMethodParams = SearchMethodParams(requests = Seq(
        SearchForHits(indexName = testIndex, query = Some(""), filters = Some("brand:Apple"))
      ))
    ))
    val hits = asSearchResponse(resp.results.head).hits
    hits should not be empty
    hits.foreach { hit =>
      hit.get("brand").map(_.toString) shouldBe Some("Apple")
    }
  }

  test("search with facets returns facet counts") {
    val resp = await(client.search(
      searchMethodParams = SearchMethodParams(requests = Seq(
        SearchForHits(indexName = testIndex, query = Some(""), facets = Some(Seq("brand", "category")))
      ))
    ))
    val result = asSearchResponse(resp.results.head)
    result.facets should not be empty
    result.facets.get.keys should contain ("brand")
    result.facets.get.keys should contain ("category")
  }

  test("search pagination limits hits per page") {
    val resp = await(client.search(
      searchMethodParams = SearchMethodParams(requests = Seq(
        SearchForHits(indexName = testIndex, query = Some(""), hitsPerPage = Some(2))
      ))
    ))
    val result = asSearchResponse(resp.results.head)
    result.hits.size should be <= 2
    result.nbPages.getOrElse(0) should be >= 2
  }

  test("multi-index search returns two result sets") {
    val resp = await(client.search(
      searchMethodParams = SearchMethodParams(requests = Seq(
        SearchForHits(indexName = testIndex, query = Some("apple")),
        SearchForHits(indexName = testIndex, query = Some("dell")),
      ))
    ))
    resp.results.size shouldBe 2
  }

  // ── Object CRUD ─────────────────────────────────────────────────────────────

  test("getObject returns correct record") {
    val obj = await(client.getObject(indexName = testIndex, objectID = "phone1"))
    obj.get("name").map(_.toString) shouldBe Some("iPhone 15 Pro")
  }

  test("addOrUpdateObject and deleteObject round-trip") {
    val tempId = "temp_scala_1"
    val saveResp = await(client.addOrUpdateObject(
      indexName = testIndex,
      objectID  = tempId,
      body      = Map("objectID" -> tempId, "name" -> "Temp Product", "brand" -> "Test", "price" -> 1),
    ))
    await(client.waitForTask(testIndex, saveResp.taskID.get))
    val obj = await(client.getObject(indexName = testIndex, objectID = tempId))
    obj.get("name").map(_.toString) shouldBe Some("Temp Product")

    await(client.deleteObject(indexName = testIndex, objectID = tempId))
  }

  // ── Settings ────────────────────────────────────────────────────────────────

  test("getSettings returns configured searchableAttributes") {
    val settings = await(client.getSettings(indexName = testIndex))
    settings.searchableAttributes should not be empty
  }

  test("setSettings update is reflected in getSettings") {
    val resp = await(client.setSettings(
      indexName = testIndex,
      indexSettings = IndexSettings(searchableAttributes = Some(Seq("name", "brand", "category", "price")))
    ))
    await(client.waitForTask(testIndex, resp.taskID))
    val settings = await(client.getSettings(indexName = testIndex))
    settings.searchableAttributes.getOrElse(Seq.empty) should contain ("price")

    // Restore
    val restoreResp = await(client.setSettings(
      indexName = testIndex,
      indexSettings = IndexSettings(searchableAttributes = Some(Seq("name", "brand", "category")))
    ))
    await(client.waitForTask(testIndex, restoreResp.taskID))
  }

  // ── Synonyms ─────────────────────────────────────────────────────────────────

  test("saveSynonym and searchSynonyms round-trip") {
    val synonymResp = await(client.saveSynonym(
      indexName = testIndex,
      objectID  = "syn_phone_scala",
      synonymHit = SynonymHit(
        objectID = "syn_phone_scala",
        `type` = SynonymType.Synonym,
        synonyms = Some(Seq("phone", "mobile", "cell")),
      )
    ))
    await(client.waitForTask(testIndex, synonymResp.taskID))

    val resp = await(client.searchSynonyms(indexName = testIndex))
    resp.hits should not be empty

    await(client.deleteSynonym(indexName = testIndex, objectID = "syn_phone_scala"))
  }

  // ── Rules ─────────────────────────────────────────────────────────────────────

  test("saveRule and searchRules round-trip") {
    val ruleResp = await(client.saveRule(
      indexName = testIndex,
      objectID  = "rule_budget_scala",
      rule = Rule(
        objectID = "rule_budget_scala",
        conditions = Some(Seq(Condition(pattern = Some("budget"), anchoring = Some(Anchoring.Contains)))),
        consequence = Consequence(params = Some(ConsequenceParams(filters = Some("price < 1000")))),
      )
    ))
    await(client.waitForTask(testIndex, ruleResp.taskID))

    val resp = await(client.searchRules(indexName = testIndex))
    resp.nbHits should be >= 1

    await(client.deleteRule(indexName = testIndex, objectID = "rule_budget_scala"))
  }

  // ── User agent (smoke test) ──────────────────────────────────────────────────

  test("client is functional (user-agent via live call)") {
    val resp = await(client.listIndices())
    resp.items should not be null
  }
}
