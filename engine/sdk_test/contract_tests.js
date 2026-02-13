import { algoliasearch } from 'algoliasearch';

function createLocalRequester() {
  return {
    async send(request) {
      const url = new URL(request.url);
      url.protocol = 'http:';
      url.host = 'localhost:7700';
      
      const response = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data
      });
      
      return {
        status: response.status,
        content: await response.text(),
        isTimedOut: false
      };
    }
  };
}

const client = algoliasearch('test-app', 'test-key', {
  requester: createLocalRequester()
});

const TEST_INDEX = 'contract_test_' + Date.now();

async function waitForIndexing(indexName, expectedCount, maxWaitMs = 5000) {
  const start = Date.now();
  while (Date.now() - start < maxWaitMs) {
    const search = await client.search({
      requests: [{ indexName, query: '', hitsPerPage: 1 }]
    });
    if (search.results[0].nbHits >= expectedCount) {
      return true;
    }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  return false;
}

async function cleanup() {
  try {
    await client.deleteIndex({ indexName: TEST_INDEX });
  } catch (e) {
  }
}

const tests = [];

function test(name, fn) {
  tests.push({ name, fn });
}

test('POST /1/indexes/{indexName}/batch - addObject', async () => {
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '1', name: 'Product 1', price: 100 },
      { objectID: '2', name: 'Product 2', price: 200 }
    ]
  });
  
  await waitForIndexing(TEST_INDEX, 2);
  
  const result = await client.search({
    requests: [{ indexName: TEST_INDEX, query: '' }]
  });
  
  if (result.results[0].nbHits !== 2) {
    throw new Error(`Expected 2 hits, got ${result.results[0].nbHits}`);
  }
});

test('POST /1/indexes/{indexName}/batch - updateObject', async () => {
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [{ objectID: '1', name: 'Updated Product', price: 150 }]
  });
  
  await waitForIndexing(TEST_INDEX, 2);
  
  const obj = await client.getObject({ indexName: TEST_INDEX, objectID: '1' });
  if (obj.name !== 'Updated Product') {
    throw new Error(`Expected 'Updated Product', got '${obj.name}'`);
  }
});

test('POST /1/indexes/{indexName}/batch - partialUpdateObject', async () => {
  await client.partialUpdateObjects({
    indexName: TEST_INDEX,
    objects: [{ objectID: '1', price: 175 }]
  });
  
  await waitForIndexing(TEST_INDEX, 2);
  
  const obj = await client.getObject({ indexName: TEST_INDEX, objectID: '1' });
  if (obj.price !== 175) {
    throw new Error(`Expected price 175, got ${obj.price}`);
  }
  if (obj.name !== 'Updated Product') {
    throw new Error(`Partial update overwrote name: ${obj.name}`);
  }
});

test('POST /1/indexes/{indexName}/batch - partialUpdateObject with createIfNotExists=false', async () => {
  try {
    await client.partialUpdateObjects({
      indexName: TEST_INDEX,
      objects: [{ objectID: '999', name: 'Should Not Create' }],
      createIfNotExists: false
    });
    
    await waitForIndexing(TEST_INDEX, 2);
    
    try {
      await client.getObject({ indexName: TEST_INDEX, objectID: '999' });
      throw new Error('Object should not have been created');
    } catch (e) {
      if (e.status !== 404) throw e;
    }
  } catch (e) {
    if (e.message.includes('should not have been created')) throw e;
  }
});

test('POST /1/indexes/{indexName}/batch - deleteObject', async () => {
  await client.deleteObjects({
    indexName: TEST_INDEX,
    objectIDs: ['2']
  });
  
  await waitForIndexing(TEST_INDEX, 1);
  
  const result = await client.search({
    requests: [{ indexName: TEST_INDEX, query: '' }]
  });
  
  if (result.results[0].nbHits !== 1) {
    throw new Error(`Expected 1 hit after delete, got ${result.results[0].nbHits}`);
  }
});

test('GET /1/indexes/{indexName}/{objectID}', async () => {
  const obj = await client.getObject({ indexName: TEST_INDEX, objectID: '1' });
  if (!obj.objectID) {
    throw new Error('Missing objectID in response');
  }
  if (!obj.name) {
    throw new Error('Missing name field');
  }
});

test('GET /1/indexes/{indexName}/{objectID} - 404 for missing object', async () => {
  try {
    await client.getObject({ indexName: TEST_INDEX, objectID: 'nonexistent' });
    throw new Error('Should have thrown 404');
  } catch (e) {
    if (e.status !== 404) {
      throw new Error(`Expected 404, got ${e.status}`);
    }
  }
});

test('PUT /1/indexes/{indexName}/{objectID}', async () => {
  await fetch('http://localhost:7700/1/indexes/' + TEST_INDEX + '/3', {
    method: 'PUT',
    headers: {
      'Content-Type': 'application/json',
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'test-app'
    },
    body: JSON.stringify({ name: 'Product 3', category: 'test' })
  });
  
  await waitForIndexing(TEST_INDEX, 2);
  
  const obj = await client.getObject({ indexName: TEST_INDEX, objectID: '3' });
  if (obj.category !== 'test') {
    throw new Error(`Expected category 'test', got '${obj.category}'`);
  }
});

test('DELETE /1/indexes/{indexName}/{objectID}', async () => {
  await client.deleteObject({ indexName: TEST_INDEX, objectID: '3' });
  
  await waitForIndexing(TEST_INDEX, 1);
  
  try {
    await client.getObject({ indexName: TEST_INDEX, objectID: '3' });
    throw new Error('Object should have been deleted');
  } catch (e) {
    if (e.status !== 404) throw e;
  }
});

test('POST /1/indexes/{indexName}/query - text search', async () => {
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '10', name: 'Gaming Laptop', category: 'electronics' },
      { objectID: '11', name: 'Office Desk', category: 'furniture' }
    ]
  });
  
  await waitForIndexing(TEST_INDEX, 3);
  
  const result = await client.search({
    requests: [{ indexName: TEST_INDEX, query: 'laptop' }]
  });
  
  if (result.results[0].nbHits !== 1) {
    throw new Error(`Expected 1 hit for 'laptop', got ${result.results[0].nbHits}`);
  }
});

test('POST /1/indexes/{indexName}/query - filters', async () => {
  await client.setSettings({
    indexName: TEST_INDEX,
    indexSettings: {
      attributesForFaceting: ['category']
    }
  });
  
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '10', name: 'Gaming Laptop', category: 'electronics' }
    ]
  });
  
  await waitForIndexing(TEST_INDEX, 4);
  
  const result = await client.search({
    requests: [{
      indexName: TEST_INDEX,
      query: '',
      filters: 'category:electronics'
    }]
  });
  
  if (result.results[0].nbHits !== 1) {
    throw new Error(`Expected 1 electronics hit, got ${result.results[0].nbHits}`);
  }
});

test('POST /1/indexes/{indexName}/query - numeric range', async () => {
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '20', name: 'Cheap Item', price: 50 },
      { objectID: '21', name: 'Expensive Item', price: 500 }
    ]
  });
  
  await waitForIndexing(TEST_INDEX, 5);
  
  const result = await client.search({
    requests: [{
      indexName: TEST_INDEX,
      query: '',
      filters: 'price >= 200'
    }]
  });
  
  if (result.results[0].nbHits !== 1) {
    throw new Error(`Expected 1 hit for price >= 200, got ${result.results[0].nbHits}`);
  }
});

test('POST /1/indexes/{indexName}/query - facets', async () => {
  await client.setSettings({
    indexName: TEST_INDEX,
    indexSettings: {
      attributesForFaceting: ['category']
    }
  });
  
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '10', name: 'Gaming Laptop', category: 'electronics' },
      { objectID: '11', name: 'Office Desk', category: 'furniture' }
    ]
  });
  
  await waitForIndexing(TEST_INDEX, 5);
  
  const result = await client.search({
    requests: [{
      indexName: TEST_INDEX,
      query: '',
      facets: ['category']
    }]
  });
  
  if (!result.results[0].facets?.category) {
    throw new Error('Missing facets in response');
  }
  
  const facets = result.results[0].facets.category;
  if (facets.electronics !== 1) {
    throw new Error(`Expected electronics:1, got ${facets.electronics}`);
  }
});

test('POST /1/indexes/*/queries - multi-index search', async () => {
  const result = await client.search({
    requests: [
      { indexName: TEST_INDEX, query: 'laptop' },
      { indexName: TEST_INDEX, query: 'desk' }
    ]
  });
  
  if (result.results.length !== 2) {
    throw new Error(`Expected 2 results, got ${result.results.length}`);
  }
});

test('POST /1/indexes/*/objects - bulk retrieval', async () => {
  const result = await client.getObjects({
    requests: [
      { indexName: TEST_INDEX, objectID: '1' },
      { indexName: TEST_INDEX, objectID: '10' },
      { indexName: TEST_INDEX, objectID: 'missing' }
    ]
  });
  
  if (result.results.length !== 3) {
    throw new Error(`Expected 3 results, got ${result.results.length}`);
  }
  
  if (result.results[0].objectID !== '1') {
    throw new Error('First object incorrect');
  }
  
  if (result.results[2] !== null) {
    throw new Error('Missing object should return null');
  }
});

test('POST /1/indexes/*/objects - attributesToRetrieve', async () => {
  const result = await client.getObjects({
    requests: [
      { indexName: TEST_INDEX, objectID: '10', attributesToRetrieve: ['name'] }
    ]
  });
  
  if (result.results[0].category !== undefined) {
    throw new Error('Category should be filtered out');
  }
  
  if (!result.results[0].name) {
    throw new Error('Name should be present');
  }
});

test('POST /1/indexes/{indexName}/browse - cursor pagination', async () => {
  const page1 = await client.browse({
    indexName: TEST_INDEX,
    browseParams: { hitsPerPage: 2 }
  });
  
  if (!page1.cursor) {
    throw new Error('Missing cursor in response');
  }
  
  if (page1.hits.length !== 2) {
    throw new Error(`Expected 2 hits, got ${page1.hits.length}`);
  }
  
  const page2 = await client.browse({
    indexName: TEST_INDEX,
    browseParams: { cursor: page1.cursor, hitsPerPage: 2 }
  });
  
  if (page2.hits.length < 1) {
    throw new Error('Second page should have hits');
  }
});

test('POST /1/indexes/{indexName}/deleteByQuery', async () => {
  await client.deleteBy({
    indexName: TEST_INDEX,
    deleteByParams: {
      filters: 'category:furniture'
    }
  });
  
  await waitForIndexing(TEST_INDEX, 4);
  
  const result = await client.search({
    requests: [{
      indexName: TEST_INDEX,
      query: '',
      filters: 'category:furniture'
    }]
  });
  
  if (result.results[0].nbHits !== 0) {
    throw new Error(`Expected 0 furniture items, got ${result.results[0].nbHits}`);
  }
});

test('POST /1/indexes/{indexName}/clear', async () => {
  await client.clearObjects({ indexName: TEST_INDEX });
  
  await waitForIndexing(TEST_INDEX, 0);
  
  const result = await client.search({
    requests: [{ indexName: TEST_INDEX, query: '' }]
  });
  
  if (result.results[0].nbHits !== 0) {
    throw new Error(`Expected 0 hits after clear, got ${result.results[0].nbHits}`);
  }
});

test('GET /1/indexes - list indices', async () => {
  const result = await client.listIndices();
  
  if (!Array.isArray(result.items)) {
    throw new Error('Expected items array');
  }
  
  const found = result.items.find(idx => idx.name === TEST_INDEX);
  if (!found) {
    throw new Error(`Test index ${TEST_INDEX} not in list`);
  }
});

test('POST /1/indexes/{indexName}/settings - set settings', async () => {
  await client.setSettings({
    indexName: TEST_INDEX,
    indexSettings: {
      attributesForFaceting: ['brand', 'category'],
      searchableAttributes: ['name', 'description'],
      ranking: ['typo', 'geo', 'words', 'filters', 'proximity', 'attribute', 'exact', 'custom']
    }
  });
  
  const settings = await client.getSettings({ indexName: TEST_INDEX });
  
  if (settings.attributesForFaceting.length !== 2) {
    throw new Error('attributesForFaceting not saved');
  }
});

test('GET /1/indexes/{indexName}/settings - get settings', async () => {
  const settings = await client.getSettings({ indexName: TEST_INDEX });
  
  if (!Array.isArray(settings.attributesForFaceting)) {
    throw new Error('Missing attributesForFaceting');
  }
});

test('POST /1/indexes/{indexName}/facets/{facetName}/query - facet search', async () => {
  await client.setSettings({
    indexName: TEST_INDEX,
    indexSettings: {
      attributesForFaceting: ['brand']
    }
  });
  
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '30', brand: 'Apple' },
      { objectID: '31', brand: 'Samsung' },
      { objectID: '32', brand: 'Apparel Co' }
    ]
  });
  
  await waitForIndexing(TEST_INDEX, 3);
  
  const response = await fetch('http://localhost:7700/1/indexes/' + TEST_INDEX + '/facets/brand/query', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'test-app'
    },
    body: JSON.stringify({ facetQuery: 'app' })
  });
  
  const result = await response.json();
  
  if (!result.facetHits) {
    throw new Error('Missing facetHits');
  }
  
  const hits = result.facetHits;
  if (hits.length !== 2) {
    throw new Error(`Expected 2 facet hits for 'app', got ${hits.length}`);
  }
});

test('DELETE /1/indexes/{indexName} - delete index', async () => {
  const tempIndex = 'temp_' + Date.now();
  
  await fetch('http://localhost:7700/1/indexes/' + tempIndex + '/batch', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'test-app'
    },
    body: JSON.stringify({
      requests: [{ action: 'addObject', body: { objectID: '1', name: 'Test' } }]
    })
  });
  
  await waitForIndexing(tempIndex, 1);
  
  await fetch('http://localhost:7700/1/indexes/' + tempIndex, {
    method: 'DELETE',
    headers: {
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'test-app'
    }
  });
  
  await new Promise(resolve => setTimeout(resolve, 100));
  
  const response = await fetch('http://localhost:7700/1/indexes/' + tempIndex + '/1', {
    method: 'GET',
    headers: {
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'test-app'
    }
  });
  
  if (response.status !== 404 && response.status !== 500) {
    throw new Error(`Expected 404 or 500, got ${response.status}`);
  }
});

async function runAllTests() {
  console.log(`\n=== Running ${tests.length} Contract Tests ===\n`);
  
  await cleanup();
  
  let passed = 0;
  let failed = 0;
  
  for (const { name, fn } of tests) {
    try {
      await fn();
      console.log(`✓ ${name}`);
      passed++;
    } catch (e) {
      console.log(`✗ ${name}`);
      console.log(`  Error: ${e.message}`);
      if (e.stack) {
        console.log(`  ${e.stack.split('\n')[1]}`);
      }
      failed++;
    }
  }
  
  console.log(`\n=== Results: ${passed} passed, ${failed} failed ===\n`);
  
  await cleanup();
  
  process.exit(failed > 0 ? 1 : 0);
}

runAllTests().catch(e => {
  console.error('Fatal error:', e);
  process.exit(1);
});