import XCTest
import Core
import Search

final class SearchE2ETests: XCTestCase {
    static let testIndex = "test_swift_sdk"
    static var client: SearchClient!

    override class func setUp() {
        super.setUp()

        let appID = ProcessInfo.processInfo.environment["FLAPJACK_APP_ID"] ?? "test-app"
        let apiKey = ProcessInfo.processInfo.environment["FLAPJACK_API_KEY"] ?? "test-api-key"
        let host = ProcessInfo.processInfo.environment["FLAPJACK_HOST"] ?? "localhost"
        let port = Int(ProcessInfo.processInfo.environment["FLAPJACK_PORT"] ?? "7700") ?? 7700

        let configuration = try! SearchClientConfiguration(
            appID: appID,
            apiKey: apiKey,
            hosts: [Host(url: host, port: port, scheme: "http", callType: .readWrite)]
        )
        client = SearchClient(configuration: configuration)

        // Seed test data
        let settings = IndexSettings(
            searchableAttributes: ["name", "brand", "category"],
            attributesForFaceting: ["brand", "category", "price"]
        )
        _ = try? client.setSettings(indexName: testIndex, indexSettings: settings)

        let objects: [BatchRequest] = [
            BatchRequest(action: .addObject, body: ["objectID": "phone1", "name": "iPhone 15 Pro", "brand": "Apple", "category": "Phone", "price": 999]),
            BatchRequest(action: .addObject, body: ["objectID": "phone2", "name": "Samsung Galaxy S24", "brand": "Samsung", "category": "Phone", "price": 799]),
            BatchRequest(action: .addObject, body: ["objectID": "laptop1", "name": "MacBook Pro M3", "brand": "Apple", "category": "Laptop", "price": 1999]),
            BatchRequest(action: .addObject, body: ["objectID": "laptop2", "name": "Google Pixel 8", "brand": "Google", "category": "Phone", "price": 699]),
            BatchRequest(action: .addObject, body: ["objectID": "laptop3", "name": "Dell XPS 15", "brand": "Dell", "category": "Laptop", "price": 1299]),
        ]
        _ = try? client.batch(indexName: testIndex, batchWriteParams: BatchWriteParams(requests: objects))

        Thread.sleep(forTimeInterval: 0.5)
    }

    // MARK: - List Indices

    func testListIndices() async throws {
        let response = try await Self.client.listIndices()
        XCTAssertNotNil(response.items)
        let found = response.items?.contains(where: { $0.name == Self.testIndex }) ?? false
        XCTAssertTrue(found, "Test index should appear in listIndices")
    }

    // MARK: - Basic Search

    func testBasicSearch() async throws {
        let response = try await Self.client.search(
            searchMethodParams: SearchMethodParams(requests: [
                SearchQuery.searchForHits(SearchForHits(query: "iPhone", indexName: Self.testIndex))
            ])
        ) as SearchResponses<[String: AnyCodable]>
        XCTAssertFalse(response.results.isEmpty)
    }

    // MARK: - Empty Query Returns All

    func testEmptyQueryReturnsAll() async throws {
        let response = try await Self.client.search(
            searchMethodParams: SearchMethodParams(requests: [
                SearchQuery.searchForHits(SearchForHits(query: "", indexName: Self.testIndex))
            ])
        ) as SearchResponses<[String: AnyCodable]>
        XCTAssertFalse(response.results.isEmpty)
    }

    // MARK: - Search With Filters

    func testSearchWithFilters() async throws {
        let response = try await Self.client.search(
            searchMethodParams: SearchMethodParams(requests: [
                SearchQuery.searchForHits(SearchForHits(query: "", filters: "brand:Apple", indexName: Self.testIndex))
            ])
        ) as SearchResponses<[String: AnyCodable]>
        XCTAssertFalse(response.results.isEmpty)
    }

    // MARK: - Search With Facets

    func testSearchWithFacets() async throws {
        let response = try await Self.client.search(
            searchMethodParams: SearchMethodParams(requests: [
                SearchQuery.searchForHits(SearchForHits(query: "", facets: ["brand", "category"], indexName: Self.testIndex))
            ])
        ) as SearchResponses<[String: AnyCodable]>
        XCTAssertFalse(response.results.isEmpty)
    }

    // MARK: - Get Object

    func testGetObject() async throws {
        let obj = try await Self.client.getObject(indexName: Self.testIndex, objectID: "phone1") as [String: AnyCodable]
        XCTAssertEqual(obj["name"]?.value as? String, "iPhone 15 Pro")
        XCTAssertEqual(obj["brand"]?.value as? String, "Apple")
    }

    // MARK: - Get Settings

    func testGetSettings() async throws {
        let settings = try await Self.client.getSettings(indexName: Self.testIndex)
        XCTAssertNotNil(settings.searchableAttributes)
        XCTAssertTrue(settings.searchableAttributes?.contains("name") ?? false)
    }

    // MARK: - Save and Search Synonyms

    func testSaveAndSearchSynonyms() async throws {
        let synonym = SynonymHit(objectID: "syn-phone-mobile", type: .synonym, synonyms: ["phone", "mobile", "smartphone"])
        _ = try await Self.client.saveSynonyms(indexName: Self.testIndex, synonymHit: [synonym])
        try await Task.sleep(nanoseconds: 500_000_000)

        let response = try await Self.client.searchSynonyms(indexName: Self.testIndex)
        XCTAssertTrue(response.nbHits >= 1)
    }

    // MARK: - User Agent Contains Flapjack

    func testUserAgentContainsFlapjack() {
        let agent = UserAgent.library
        XCTAssertTrue(agent.title.contains("Flapjack"), "User-Agent should contain 'Flapjack', got: \(agent.title)")
    }
}
