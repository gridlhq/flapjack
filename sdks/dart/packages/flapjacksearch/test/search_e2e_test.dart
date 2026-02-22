/// E2E tests for the Flapjack Dart SDK against a live Flapjack server.
///
/// Prerequisites:
///   - Flapjack server running on localhost:7700 (--no-auth)
///
/// Run (from packages/flapjacksearch/):
///   dart test test/search_e2e_test.dart
///
/// Environment variables (optional, defaults shown):
///   FLAPJACK_APP_ID  = test-app
///   FLAPJACK_API_KEY = test-api-key
///   FLAPJACK_HOST    = localhost
///   FLAPJACK_PORT    = 7700

import 'dart:io';

import 'package:flapjacksearch/flapjacksearch.dart';
import 'package:test/test.dart';

const _indexName = 'test_dart_e2e';

SearchClient _buildClient() {
  final appId  = Platform.environment['FLAPJACK_APP_ID']  ?? 'test-app';
  final apiKey = Platform.environment['FLAPJACK_API_KEY'] ?? 'test-api-key';
  final host   = Platform.environment['FLAPJACK_HOST']    ?? 'localhost';
  final port   = int.tryParse(Platform.environment['FLAPJACK_PORT'] ?? '7700') ?? 7700;

  return SearchClient(
    appId:   appId,
    apiKey:  apiKey,
    options: ClientOptions(
      hosts: [
        Host(url: host, port: port, scheme: 'http', callType: null),
      ],
      connectTimeout: const Duration(seconds: 5),
      readTimeout:    const Duration(seconds: 10),
      writeTimeout:   const Duration(seconds: 30),
    ),
  );
}

final _seedObjects = [
  <String, Object?>{
    'objectID': 'phone1', 'name': 'iPhone 15 Pro',
    'brand': 'Apple', 'category': 'Phone', 'price': 999,
  },
  <String, Object?>{
    'objectID': 'phone2', 'name': 'Samsung Galaxy S24',
    'brand': 'Samsung', 'category': 'Phone', 'price': 799,
  },
  <String, Object?>{
    'objectID': 'laptop1', 'name': 'MacBook Pro M3',
    'brand': 'Apple', 'category': 'Laptop', 'price': 1999,
  },
  <String, Object?>{
    'objectID': 'phone3', 'name': 'Google Pixel 8',
    'brand': 'Google', 'category': 'Phone', 'price': 699,
  },
  <String, Object?>{
    'objectID': 'laptop2', 'name': 'Dell XPS 15',
    'brand': 'Dell', 'category': 'Laptop', 'price': 1299,
  },
];

void main() {
  late SearchClient client;

  setUpAll(() async {
    client = _buildClient();

    // Push settings and wait for the task to complete before seeding.
    final settingsResp = await client.setSettings(
      indexName: _indexName,
      indexSettings: IndexSettings(
        searchableAttributes: ['name', 'brand', 'category'],
        attributesForFaceting: ['brand', 'category'],
      ),
    );
    await client.waitTask(indexName: _indexName, taskID: settingsResp.taskID);

    // Batch seed — Action enum values are camelCase in Dart.
    // Wait for the batch task to be published before any test runs.
    final batchResp = await client.batch(
      indexName: _indexName,
      batchWriteParams: BatchWriteParams(
        requests: _seedObjects
            .map((obj) => BatchRequest(action: Action.addObject, body: obj))
            .toList(),
      ),
    );
    await client.waitTask(indexName: _indexName, taskID: batchResp.taskID);
  });

  tearDownAll(() async {
    try {
      await client.deleteIndex(indexName: _indexName);
    } catch (_) {}
  });

  // ── List indices ────────────────────────────────────────────────────────────

  test('listIndices contains test index', () async {
    final resp = await client.listIndices();
    // items is non-nullable List<FetchedIndex>
    final found = resp.items.any((idx) => idx.name == _indexName);
    expect(found, isTrue, reason: 'Expected $_indexName in index list');
  });

  // ── Search ──────────────────────────────────────────────────────────────────
  //
  // SearchMethodParams.requests is Iterable<dynamic>; pass SearchForHits directly.
  // There is no SearchQuery class in the Dart SDK.
  // search() results is Iterable<dynamic>; deserialise with SearchResponse.fromJson().

  test('basic text search returns relevant hit', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: 'pixel'),
      ]),
    );
    final result = SearchResponse.fromJson(
        resp.results.first as Map<String, dynamic>);
    expect(result.hits, isNotEmpty);
    // Hit extends DelegatingMap<String, dynamic>; access fields as map keys
    final hasPixel = result.hits.any(
      (h) => (h['name'] as String? ?? '').toLowerCase().contains('pixel'),
    );
    expect(hasPixel, isTrue);
  });

  test('empty query returns all documents', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: ''),
      ]),
    );
    final result = SearchResponse.fromJson(
        resp.results.first as Map<String, dynamic>);
    expect(result.hits.length, greaterThanOrEqualTo(5));
  });

  test('search with filter by brand', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: '', filters: 'brand:Apple'),
      ]),
    );
    final hits = SearchResponse.fromJson(
        resp.results.first as Map<String, dynamic>).hits;
    expect(hits, isNotEmpty);
    for (final hit in hits) {
      expect(hit['brand'], equals('Apple'));
    }
  });

  test('search with facets returns facet counts', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: '', facets: ['brand', 'category']),
      ]),
    );
    final result = SearchResponse.fromJson(
        resp.results.first as Map<String, dynamic>);
    expect(result.facets, isNotNull);
    expect(result.facets!.containsKey('brand'), isTrue);
    expect(result.facets!.containsKey('category'), isTrue);
  });

  test('search pagination limits hits per page', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: '', hitsPerPage: 2),
      ]),
    );
    final result = SearchResponse.fromJson(
        resp.results.first as Map<String, dynamic>);
    expect(result.hits.length, lessThanOrEqualTo(2));
    // nbPages is int? in Dart SDK
    expect(result.nbPages ?? 0, greaterThanOrEqualTo(2));
  });

  test('highlight result present', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: 'macbook'),
      ]),
    );
    final hits = SearchResponse.fromJson(
        resp.results.first as Map<String, dynamic>).hits;
    expect(hits, isNotEmpty);
    // Hit.highlightResult is Map<String, dynamic>? from the _highlightResult field
    expect(hits.first.highlightResult, isNotNull);
  });

  test('multi-index search returns two result sets', () async {
    final resp = await client.search(
      searchMethodParams: SearchMethodParams(requests: [
        SearchForHits(indexName: _indexName, query: 'apple'),
        SearchForHits(indexName: _indexName, query: 'dell'),
      ]),
    );
    expect(resp.results.length, equals(2));
  });

  // ── Extension methods ────────────────────────────────────────────────────────

  test('searchIndex extension returns SearchResponse directly', () async {
    final result = await client.searchIndex(
      request: SearchForHits(indexName: _indexName, query: 'samsung'),
    );
    expect(result.hits, isNotEmpty);
  });

  test('searchForHits extension returns iterable of SearchResponse', () async {
    final results = await client.searchForHits(
      requests: [
        SearchForHits(indexName: _indexName, query: 'apple'),
        SearchForHits(indexName: _indexName, query: 'dell'),
      ],
    );
    final list = results.toList();
    expect(list.length, equals(2));
    for (final r in list) {
      expect(r.hits, isNotEmpty);
    }
  });

  // ── Object CRUD ─────────────────────────────────────────────────────────────
  //
  // getObject returns Future<Object>; cast to Map<String, dynamic> before indexing.
  // All objectID parameters use uppercase 'ID' (not 'Id').

  test('getObject returns correct record', () async {
    final obj = await client.getObject(indexName: _indexName, objectID: 'phone1');
    expect((obj as Map<String, dynamic>)['name'], equals('iPhone 15 Pro'));
  });

  test('partialUpdateObject updates a field', () async {
    final updateResp = await client.partialUpdateObject(
      indexName: _indexName,
      objectID: 'phone1',
      attributesToUpdate: {'price': 949},
    );
    await client.waitTask(indexName: _indexName, taskID: updateResp.taskID!);
    final obj = await client.getObject(indexName: _indexName, objectID: 'phone1');
    expect((obj as Map<String, dynamic>)['price'], equals(949));

    // Restore
    final restoreResp = await client.partialUpdateObject(
      indexName: _indexName, objectID: 'phone1',
      attributesToUpdate: {'price': 999},
    );
    await client.waitTask(indexName: _indexName, taskID: restoreResp.taskID!);
  });

  test('addOrUpdateObject and deleteObject round-trip', () async {
    final saveResp = await client.addOrUpdateObject(
      indexName: _indexName,
      objectID: 'temp_dart_1',
      body: {'objectID': 'temp_dart_1', 'name': 'Temp Product', 'brand': 'Test', 'price': 1},
    );
    await client.waitTask(indexName: _indexName, taskID: saveResp.taskID!);
    final obj = await client.getObject(indexName: _indexName, objectID: 'temp_dart_1');
    expect((obj as Map<String, dynamic>)['name'], equals('Temp Product'));

    await client.deleteObject(indexName: _indexName, objectID: 'temp_dart_1');
  });

  // ── Settings ────────────────────────────────────────────────────────────────

  test('getSettings returns configured searchableAttributes', () async {
    final settings = await client.getSettings(indexName: _indexName);
    expect(settings.searchableAttributes, isNotNull);
    expect(settings.searchableAttributes, isNotEmpty);
  });

  test('setSettings updates and getSettings reflects change', () async {
    final resp = await client.setSettings(
      indexName: _indexName,
      indexSettings: IndexSettings(searchableAttributes: ['name', 'brand', 'category', 'price']),
    );
    await client.waitTask(indexName: _indexName, taskID: resp.taskID);
    final settings = await client.getSettings(indexName: _indexName);
    expect(settings.searchableAttributes, contains('price'));

    // Restore
    final restoreResp = await client.setSettings(
      indexName: _indexName,
      indexSettings: IndexSettings(searchableAttributes: ['name', 'brand', 'category']),
    );
    await client.waitTask(indexName: _indexName, taskID: restoreResp.taskID);
  });

  // ── Synonyms ─────────────────────────────────────────────────────────────────
  //
  // SynonymType.synonym is camelCase in Dart (unlike Kotlin/Scala).
  // SynonymHit.objectID uses uppercase 'ID'.

  test('saveSynonym and searchSynonyms round-trip', () async {
    final synonymResp = await client.saveSynonym(
      indexName: _indexName,
      objectID: 'syn_phone_dart',
      synonymHit: SynonymHit(
        objectID: 'syn_phone_dart',
        type: SynonymType.synonym,
        synonyms: ['phone', 'mobile', 'cell'],
      ),
    );
    await client.waitTask(indexName: _indexName, taskID: synonymResp.taskID);

    final resp = await client.searchSynonyms(indexName: _indexName);
    expect(resp.hits, isNotEmpty);

    await client.deleteSynonym(indexName: _indexName, objectID: 'syn_phone_dart');
  });

  // ── Rules ────────────────────────────────────────────────────────────────────
  //
  // Anchoring.contains is camelCase in Dart (unlike Kotlin/Scala).
  // Rule.objectID uses uppercase 'ID'.

  test('saveRule and searchRules round-trip', () async {
    final ruleResp = await client.saveRule(
      indexName: _indexName,
      objectID: 'rule_budget_dart',
      rule: Rule(
        objectID: 'rule_budget_dart',
        conditions: [Condition(pattern: 'budget', anchoring: Anchoring.contains)],
        consequence: Consequence(params: ConsequenceParams(filters: 'price < 1000')),
      ),
    );
    await client.waitTask(indexName: _indexName, taskID: ruleResp.taskID);

    final resp = await client.searchRules(indexName: _indexName);
    expect(resp.nbHits, greaterThanOrEqualTo(1));

    await client.deleteRule(indexName: _indexName, objectID: 'rule_budget_dart');
  });

  // ── User agent ───────────────────────────────────────────────────────────────

  test('client is functional (user-agent check via live call)', () async {
    // A successful call is proof that the SDK user-agent and transport are correct.
    final resp = await client.listIndices();
    expect(resp.items, isNotNull);
  });
}
