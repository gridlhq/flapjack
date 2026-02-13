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

async function runTests() {
  const TEST_INDEX = 'test_' + Date.now();
  let failed = false;
  
  console.log(`\n=== Using index: ${TEST_INDEX} ===`);
  
  console.log('\n=== Test 1: Settings ===');
  try {
    await client.setSettings({
      indexName: TEST_INDEX,
      indexSettings: {
        attributesForFaceting: ['category', 'brand']
      }
    });
    console.log('✓ Settings configured');
  } catch (e) {
    console.log('✗ Settings failed:', e.message);
  }
  
  console.log('\n=== Test 2: Batch Upload ===');
  try {
    const uploadResult = await client.saveObjects({
      indexName: TEST_INDEX,
      objects: [
        { objectID: '1', name: 'Gaming Laptop', category: 'electronics', brand: 'Dell', price: 999 },
        { objectID: '2', name: 'Office Laptop', category: 'electronics', brand: 'HP', price: 599 },
        { objectID: '3', name: 'Tablet', category: 'electronics', brand: 'Apple', price: 499 }
      ]
    });
    console.log('✓ Upload:', uploadResult);
    
    console.log('\nWaiting for indexing...');
    const taskID = uploadResult[0].taskID;
    for (let i = 0; i < 50; i++) {
      await new Promise(resolve => setTimeout(resolve, 50));
      const search = await client.search({
        requests: [{ indexName: TEST_INDEX, query: '', hitsPerPage: 1 }]
      });
      if (search.results[0].nbHits >= 3) {
        console.log(`✓ Indexing complete (${i * 50}ms)`);
        break;
      }
    }
  } catch (e) {
    console.log('✗ Upload failed:', e.message);
  }
  
  console.log('\n=== Test 3: Text Search ===');
  try {
    const searchResult = await client.search({
      requests: [{
        indexName: TEST_INDEX,
        query: 'laptop'
      }]
    });
    const hits = searchResult.results[0];
    if (hits.nbHits !== 2) {
      throw new Error(`Expected 2 hits for "laptop", got ${hits.nbHits}`);
    }
    console.log('✓ Search hits:', hits.nbHits);
    console.log('  Results:', hits.hits.map(h => h.name));
  } catch (e) {
    console.log('✗ Search failed:', e.message);
    failed = true;
  }
  
  console.log('\n=== Test 4: Numeric Filter ===');
  try {
    const filterResult = await client.search({
      requests: [{
        indexName: TEST_INDEX,
        query: '',
        filters: 'price >= 600'
      }]
    });
    const hits = filterResult.results[0];
    console.log('✓ Filter hits:', hits.nbHits);
    console.log('  Results:', hits.hits.map(h => `${h.name} ($${h.price})`));
  } catch (e) {
    console.log('✗ Filter failed:', e.message);
  }
  
  console.log('\n=== Test 5: Facet Filter ===');
  try {
    const facetFilterResult = await client.search({
      requests: [{
        indexName: TEST_INDEX,
        query: '',
        filters: 'category:electronics'
      }]
    });
    const hits = facetFilterResult.results[0];
    console.log('✓ Facet filter hits:', hits.nbHits);
  } catch (e) {
    console.log('✗ Facet filter failed:', e.message);
  }
  
  console.log('\n=== Test 6: Facet Aggregation ===');
  try {
    const facetResult = await client.search({
      requests: [{
        indexName: TEST_INDEX,
        query: '',
        facets: ['category', 'brand']
      }]
    });
    const hits = facetResult.results[0];
    console.log('✓ Facets returned:', Object.keys(hits.facets || {}));
    console.log('  Category counts:', hits.facets?.category);
    console.log('  Brand counts:', hits.facets?.brand);
  } catch (e) {
    console.log('✗ Facet aggregation failed:', e.message);
  }
  
  console.log('\n=== Test 7: Complex Filter ===');
  try {
    const complexResult = await client.search({
      requests: [{
        indexName: TEST_INDEX,
        query: 'laptop',
        filters: '(category:electronics OR category:computers) AND price >= 500'
      }]
    });
    const hits = complexResult.results[0];
    console.log('✓ Complex filter hits:', hits.nbHits);
  } catch (e) {
    console.log('✗ Complex filter failed:', e.message);
  }
  
  console.log('\n=== Test 8: Get Settings ===');
  try {
    const settings = await client.getSettings({ indexName: TEST_INDEX });
    console.log('✓ Settings retrieved:', settings.attributesForFaceting);
  } catch (e) {
    console.log('✗ Get settings failed:', e.message);
    failed = true;
  }
  
  if (failed) {
    console.log('\n❌ Some tests failed');
    process.exit(1);
  } else {
    console.log('\n✅ All tests passed');
    process.exit(0);
  }
}

runTests().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});