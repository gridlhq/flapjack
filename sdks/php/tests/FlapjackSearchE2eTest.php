<?php

namespace Flapjack\FlapjackSearch\Tests;

use Flapjack\FlapjackSearch\Api\SearchClient;
use Flapjack\FlapjackSearch\Configuration\SearchConfig;
use Flapjack\FlapjackSearch\RetryStrategy\ClusterHosts;
use PHPUnit\Framework\TestCase;

/**
 * End-to-end tests for Flapjack PHP SDK against a local Flapjack server.
 *
 * Prerequisites:
 *   - Flapjack server running on localhost:7700
 *   - Environment vars: FLAPJACK_APP_ID, FLAPJACK_API_KEY (or defaults below)
 */
class FlapjackSearchE2eTest extends TestCase
{
    private static ?SearchClient $client = null;
    private static string $indexName = 'test_php_sdk';
    private static string $appId;
    private static string $apiKey;

    public static function setUpBeforeClass(): void
    {
        self::$appId = getenv('FLAPJACK_APP_ID') ?: 'test-app';
        self::$apiKey = getenv('FLAPJACK_API_KEY') ?: 'test-api-key';

        $config = SearchConfig::create(self::$appId, self::$apiKey);
        $config->setFullHosts(['http://localhost:7700']);
        $config->setConnectTimeout(5);
        $config->setReadTimeout(10);
        $config->setWriteTimeout(30);

        self::$client = SearchClient::createWithConfig($config);

        // Seed test data
        $objects = [
            ['objectID' => 'phone1', 'name' => 'iPhone 15 Pro', 'brand' => 'Apple', 'category' => 'Phone', 'price' => 999],
            ['objectID' => 'phone2', 'name' => 'Samsung Galaxy S24', 'brand' => 'Samsung', 'category' => 'Phone', 'price' => 799],
            ['objectID' => 'laptop1', 'name' => 'MacBook Pro M3', 'brand' => 'Apple', 'category' => 'Laptop', 'price' => 1999],
            ['objectID' => 'laptop2', 'name' => 'Google Pixel 8', 'brand' => 'Google', 'category' => 'Phone', 'price' => 699],
            ['objectID' => 'laptop3', 'name' => 'Dell XPS 15', 'brand' => 'Dell', 'category' => 'Laptop', 'price' => 1299],
        ];

        // Configure settings first
        $settingsResponse = self::$client->setSettings(self::$indexName, [
            'searchableAttributes' => ['name', 'brand', 'category'],
            'attributesForFaceting' => ['brand', 'category', 'price'],
        ]);
        self::waitForTask($settingsResponse['taskID'] ?? 0);

        // Save objects
        $saveResponse = self::$client->saveObjects(self::$indexName, $objects);
        if (isset($saveResponse[0]['taskID'])) {
            self::waitForTask($saveResponse[0]['taskID']);
        }

        // Give the index a moment to settle
        usleep(500000);
    }

    public static function tearDownAfterClass(): void
    {
        if (self::$client) {
            try {
                self::$client->deleteIndex(self::$indexName);
            } catch (\Exception $e) {
                // Ignore cleanup errors
            }
        }
    }

    private static function waitForTask(int $taskId, int $maxRetries = 30): void
    {
        if ($taskId <= 0) {
            return;
        }

        for ($i = 0; $i < $maxRetries; $i++) {
            try {
                $response = self::$client->getTask(self::$indexName, $taskId);
                if (isset($response['status']) && $response['status'] === 'published') {
                    return;
                }
            } catch (\Exception $e) {
                // Ignore and retry
            }
            usleep(200000); // 200ms
        }
    }

    // =========================================================================
    // List Indices
    // =========================================================================

    public function testListIndices(): void
    {
        $response = self::$client->listIndices();
        $this->assertArrayHasKey('items', $response);

        $indexNames = array_map(fn($item) => $item['name'], $response['items']);
        $this->assertContains(self::$indexName, $indexNames);
    }

    // =========================================================================
    // Search Tests
    // =========================================================================

    public function testBasicSearch(): void
    {
        $response = self::$client->search([
            'requests' => [
                ['indexName' => self::$indexName, 'query' => 'pixel'],
            ],
        ]);

        $this->assertArrayHasKey('results', $response);
        $this->assertGreaterThan(0, count($response['results']));
        $hits = $response['results'][0]['hits'] ?? [];
        $this->assertGreaterThan(0, count($hits));

        $hitNames = array_map(fn($h) => $h['name'], $hits);
        $found = false;
        foreach ($hitNames as $name) {
            if (stripos($name, 'pixel') !== false) {
                $found = true;
                break;
            }
        }
        $this->assertTrue($found, 'Expected a hit containing "pixel"');
    }

    public function testEmptyQueryReturnsAll(): void
    {
        $response = self::$client->search([
            'requests' => [
                ['indexName' => self::$indexName, 'query' => ''],
            ],
        ]);

        $hits = $response['results'][0]['hits'] ?? [];
        $this->assertGreaterThanOrEqual(5, count($hits));
    }

    public function testSearchWithFilters(): void
    {
        $response = self::$client->search([
            'requests' => [
                [
                    'indexName' => self::$indexName,
                    'query' => '',
                    'filters' => 'brand:Apple',
                ],
            ],
        ]);

        $hits = $response['results'][0]['hits'] ?? [];
        $this->assertGreaterThan(0, count($hits));
        foreach ($hits as $hit) {
            $this->assertEquals('Apple', $hit['brand']);
        }
    }

    public function testSearchWithFacets(): void
    {
        $response = self::$client->search([
            'requests' => [
                [
                    'indexName' => self::$indexName,
                    'query' => '',
                    'facets' => ['brand', 'category'],
                ],
            ],
        ]);

        $this->assertArrayHasKey('facets', $response['results'][0]);
        $facets = $response['results'][0]['facets'];
        $this->assertArrayHasKey('brand', $facets);
        $this->assertArrayHasKey('category', $facets);
    }

    public function testSearchHighlighting(): void
    {
        $response = self::$client->search([
            'requests' => [
                ['indexName' => self::$indexName, 'query' => 'macbook'],
            ],
        ]);

        $hits = $response['results'][0]['hits'] ?? [];
        $this->assertGreaterThan(0, count($hits));
        $this->assertArrayHasKey('_highlightResult', $hits[0]);
    }

    public function testSearchPagination(): void
    {
        $response = self::$client->search([
            'requests' => [
                [
                    'indexName' => self::$indexName,
                    'query' => '',
                    'hitsPerPage' => 2,
                ],
            ],
        ]);

        $result = $response['results'][0];
        $this->assertLessThanOrEqual(2, count($result['hits']));
        $this->assertArrayHasKey('nbPages', $result);
        $this->assertGreaterThan(1, $result['nbPages']);
    }

    public function testMultiIndexSearch(): void
    {
        $response = self::$client->search([
            'requests' => [
                ['indexName' => self::$indexName, 'query' => 'apple'],
                ['indexName' => self::$indexName, 'query' => 'dell'],
            ],
        ]);

        $this->assertCount(2, $response['results']);
    }

    // =========================================================================
    // Object Tests
    // =========================================================================

    public function testGetObject(): void
    {
        $response = self::$client->getObject(self::$indexName, 'phone1');
        $this->assertEquals('phone1', $response['objectID']);
        $this->assertEquals('iPhone 15 Pro', $response['name']);
        $this->assertEquals('Apple', $response['brand']);
    }

    public function testPartialUpdateObject(): void
    {
        $updateResponse = self::$client->partialUpdateObject(
            self::$indexName,
            'phone1',
            ['price' => 949],
            true
        );
        self::waitForTask($updateResponse['taskID'] ?? 0);

        $obj = self::$client->getObject(self::$indexName, 'phone1');
        $this->assertEquals(949, $obj['price']);

        // Restore
        $restoreResponse = self::$client->partialUpdateObject(
            self::$indexName,
            'phone1',
            ['price' => 999],
            true
        );
        self::waitForTask($restoreResponse['taskID'] ?? 0);
    }

    public function testSaveAndDeleteObject(): void
    {
        $tempObj = ['name' => 'Temp Product', 'brand' => 'Test', 'category' => 'Test', 'price' => 1];

        $saveResponse = self::$client->addOrUpdateObject(self::$indexName, 'temp1', $tempObj);
        self::waitForTask($saveResponse['taskID'] ?? 0);

        $obj = self::$client->getObject(self::$indexName, 'temp1');
        $this->assertEquals('Temp Product', $obj['name']);

        $deleteResponse = self::$client->deleteObject(self::$indexName, 'temp1');
        self::waitForTask($deleteResponse['taskID'] ?? 0);
    }

    // =========================================================================
    // Settings Tests
    // =========================================================================

    public function testGetSettings(): void
    {
        $settings = self::$client->getSettings(self::$indexName);
        $this->assertArrayHasKey('searchableAttributes', $settings);
    }

    public function testUpdateSettings(): void
    {
        $updateResponse = self::$client->setSettings(self::$indexName, [
            'searchableAttributes' => ['name', 'brand', 'category', 'price'],
        ]);
        self::waitForTask($updateResponse['taskID'] ?? 0);

        $settings = self::$client->getSettings(self::$indexName);
        $this->assertContains('price', $settings['searchableAttributes']);

        // Restore original settings
        $restoreResponse = self::$client->setSettings(self::$indexName, [
            'searchableAttributes' => ['name', 'brand', 'category'],
        ]);
        self::waitForTask($restoreResponse['taskID'] ?? 0);
    }

    // =========================================================================
    // Synonyms Tests
    // =========================================================================

    public function testSaveAndSearchSynonyms(): void
    {
        $synonym = [
            'objectID' => 'syn_phone_mobile',
            'type' => 'synonym',
            'synonyms' => ['phone', 'mobile', 'cell'],
        ];

        $saveResponse = self::$client->saveSynonym(
            self::$indexName,
            'syn_phone_mobile',
            $synonym,
            true
        );
        self::waitForTask($saveResponse['taskID'] ?? 0);

        $searchResponse = self::$client->searchSynonyms(self::$indexName, [
            'query' => 'phone',
        ]);
        $this->assertArrayHasKey('hits', $searchResponse);

        // Cleanup
        $deleteResponse = self::$client->deleteSynonym(
            self::$indexName,
            'syn_phone_mobile',
            true
        );
        self::waitForTask($deleteResponse['taskID'] ?? 0);
    }

    // =========================================================================
    // Rules Tests
    // =========================================================================

    public function testSaveAndSearchRules(): void
    {
        $rule = [
            'objectID' => 'rule_budget',
            'condition' => [
                'pattern' => 'budget',
                'anchoring' => 'contains',
            ],
            'consequence' => [
                'params' => [
                    'filters' => 'price < 1000',
                ],
            ],
        ];

        $saveResponse = self::$client->saveRule(
            self::$indexName,
            'rule_budget',
            $rule,
            true
        );
        self::waitForTask($saveResponse['taskID'] ?? 0);

        $searchResponse = self::$client->searchRules(self::$indexName, [
            'query' => 'budget',
        ]);
        $this->assertArrayHasKey('hits', $searchResponse);

        // Cleanup
        $deleteResponse = self::$client->deleteRule(
            self::$indexName,
            'rule_budget',
            true
        );
        self::waitForTask($deleteResponse['taskID'] ?? 0);
    }

    // =========================================================================
    // User Agent Tests
    // =========================================================================

    public function testUserAgentContainsFlapjack(): void
    {
        $config = SearchConfig::create(self::$appId, self::$apiKey);
        $agent = $config->getFlapjackAgent();
        // Agent may not be set at config level - check the FlapjackAgent class
        $agentValue = \Flapjack\FlapjackSearch\Support\FlapjackAgent::get('Search');
        $this->assertStringContainsString('Flapjack for PHP', $agentValue);
    }

    public function testAddCustomUserAgent(): void
    {
        \Flapjack\FlapjackSearch\Support\FlapjackAgent::addFlapjackAgent('Search', 'MyApp', '1.0.0');
        $agent = \Flapjack\FlapjackSearch\Support\FlapjackAgent::get('Search');
        $this->assertStringContainsString('MyApp (1.0.0)', $agent);
    }
}
