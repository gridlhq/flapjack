import { algoliasearch } from 'algoliasearch';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

dotenv.config({ path: join(__dirname, '..', '.secret', '.env.secret') });

const ALGOLIA_APP_ID = process.env.ALGOLIA_APP_ID;
const ALGOLIA_API_KEY = process.env.ALGOLIA_ADMIN_KEY;

const algoliaClient = algoliasearch(ALGOLIA_APP_ID, ALGOLIA_API_KEY);
const flapjackClient = algoliasearch('test-app', 'test-key', {
  requester: {
    async send(request) {
      const url = new URL(request.url);
      url.protocol = 'http:';
      url.host = 'localhost:7700';
      
      const response = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data
      });
      
      const content = await response.text();
      return {
        status: response.status,
        content: content,
        isTimedOut: false,
        headers: Object.fromEntries(response.headers.entries())
      };
    }
  }
});

async function testExhaustiveFields() {
  const testIndex = 'test_exhaustive_debug';
  const docs = [
    { objectID: '1', name: 'Product A', category: 'electronics' },
    { objectID: '2', name: 'Product B', category: 'books' }
  ];
  
  // Setup
  await flapjackClient.setSettings({
    indexName: testIndex,
    indexSettings: { attributesForFaceting: ['category'] }
  });
  
  const [[algoliaTask]] = await Promise.all([
    algoliaClient.saveObjects({ indexName: testIndex, objects: docs }),
    flapjackClient.saveObjects({ indexName: testIndex, objects: docs })
  ]);
  
  await algoliaClient.waitForTask({ indexName: testIndex, taskID: algoliaTask.taskID });
  await new Promise(r => setTimeout(r, 3000));
  
  // Test 1: Query without facets
  console.log('\n=== TEST 1: Query without facets ===');
  const [a1, f1] = await Promise.all([
    algoliaClient.search({ requests: [{ indexName: testIndex, query: '' }] }),
    flapjackClient.search({ requests: [{ indexName: testIndex, query: '' }] })
  ]);
  
  console.log('Algolia exhaustive keys:', Object.keys(a1.results[0].exhaustive));
  console.log('Flapjack exhaustive keys:', Object.keys(f1.results[0].exhaustive));
  console.log('Algolia has exhaustiveFacetsCount?', 'exhaustiveFacetsCount' in a1.results[0]);
  console.log('Flapjack has exhaustiveFacetsCount?', 'exhaustiveFacetsCount' in f1.results[0]);
  console.log('Algolia has facets?', 'facets' in a1.results[0]);
  console.log('Flapjack has facets?', 'facets' in f1.results[0]);
  
  // Test 2: Query with facets
  console.log('\n=== TEST 2: Query with facets ===');
  const [a2, f2] = await Promise.all([
    algoliaClient.search({ requests: [{ indexName: testIndex, query: '', facets: ['category'] }] }),
    flapjackClient.search({ requests: [{ indexName: testIndex, query: '', facets: ['category'] }] })
  ]);
  
  console.log('Algolia exhaustive keys:', Object.keys(a2.results[0].exhaustive));
  console.log('Flapjack exhaustive keys:', Object.keys(f2.results[0].exhaustive));
  console.log('Algolia has exhaustiveFacetsCount?', 'exhaustiveFacetsCount' in a2.results[0]);
  console.log('Flapjack has exhaustiveFacetsCount?', 'exhaustiveFacetsCount' in f2.results[0]);
  console.log('Algolia facets:', JSON.stringify(a2.results[0].facets));
  console.log('Flapjack facets:', JSON.stringify(f2.results[0].facets));
  
  // Test 3: Query with facets but 0 results
  console.log('\n=== TEST 3: Query with facets, 0 results ===');
  const [a3, f3] = await Promise.all([
    algoliaClient.search({ requests: [{ indexName: testIndex, query: 'nonexistent', facets: ['category'] }] }),
    flapjackClient.search({ requests: [{ indexName: testIndex, query: 'nonexistent', facets: ['category'] }] })
  ]);
  
  console.log('Algolia nbHits:', a3.results[0].nbHits);
  console.log('Flapjack nbHits:', f3.results[0].nbHits);
  console.log('Algolia facets:', JSON.stringify(a3.results[0].facets));
  console.log('Flapjack facets:', JSON.stringify(f3.results[0].facets));
  console.log('Algolia has exhaustiveFacetsCount?', 'exhaustiveFacetsCount' in a3.results[0]);
  console.log('Flapjack has exhaustiveFacetsCount?', 'exhaustiveFacetsCount' in f3.results[0]);
  
  // Cleanup
  await Promise.all([
    algoliaClient.deleteIndex({ indexName: testIndex }),
    flapjackClient.deleteIndex({ indexName: testIndex })
  ]);
}

testExhaustiveFields().catch(console.error);