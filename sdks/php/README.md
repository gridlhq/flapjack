# Flapjack Search PHP SDK

A fully-featured and blazing-fast PHP API client for [Flapjack Search](https://github.com/flapjackhq). Drop-in replacement for the Algolia PHP client.

## Installation

```bash
composer require flapjackhq/flapjack-search-php:0.1.0-beta.1
```

## Quick Start

```php
use Flapjack\FlapjackSearch\Api\SearchClient;
use Flapjack\FlapjackSearch\Configuration\SearchConfig;

// Flapjack Cloud
$client = SearchClient::create('YOUR_APP_ID', 'YOUR_API_KEY');

// Self-hosted Flapjack
$config = SearchConfig::create('YOUR_APP_ID', 'YOUR_API_KEY');
$config->setFullHosts([
    ['url' => 'localhost:7700', 'accept' => 3, 'protocol' => 'http'],
]);
$client = SearchClient::createWithConfig($config);
```

## Usage

### Index Objects

```php
$objects = [
    ['objectID' => '1', 'name' => 'iPhone 15', 'brand' => 'Apple', 'price' => 999],
    ['objectID' => '2', 'name' => 'Galaxy S24', 'brand' => 'Samsung', 'price' => 799],
];

$response = $client->saveObjects('products', $objects);
```

### Search

```php
// Basic search
$response = $client->search([
    'requests' => [
        ['indexName' => 'products', 'query' => 'iphone'],
    ],
]);

// Search with filters and facets
$response = $client->search([
    'requests' => [
        [
            'indexName' => 'products',
            'query' => 'phone',
            'filters' => 'brand:Apple',
            'facets' => ['brand', 'category'],
        ],
    ],
]);
```

### Manage Settings

```php
$client->setSettings('products', [
    'searchableAttributes' => ['name', 'brand', 'category'],
    'attributesForFaceting' => ['brand', 'category', 'price'],
]);

$settings = $client->getSettings('products');
```

### Synonyms

```php
$client->saveSynonym('products', 'syn1', [
    'objectID' => 'syn1',
    'type' => 'synonym',
    'synonyms' => ['phone', 'mobile', 'cell'],
], true);
```

### Rules

```php
$client->saveRule('products', 'rule1', [
    'objectID' => 'rule1',
    'condition' => ['pattern' => 'budget', 'anchoring' => 'contains'],
    'consequence' => ['params' => ['filters' => 'price < 500']],
], true);
```

## Environment Variables

The SDK checks for credentials in this order:

1. Constructor parameters
2. `FLAPJACK_APP_ID` / `FLAPJACK_API_KEY`
3. `ALGOLIA_APP_ID` / `ALGOLIA_API_KEY` (backward compatibility)

## Requirements

- PHP >= 8.1 (except 8.3.0)
- ext-curl
- ext-json
- ext-mbstring

## Migration from Algolia

See [MIGRATION.md](MIGRATION.md) for a step-by-step guide to migrate from the Algolia PHP SDK.

## Contributing

This repository hosts the generated Flapjack API client for PHP. To contribute, head over to the [SDK Automation repository](https://github.com/flapjackhq/sdk-automation).

## License

MIT - see [LICENSE](LICENSE) for details.
