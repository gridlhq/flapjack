import { algoliasearch } from 'algoliasearch';

function createLocalRequester() {
  return {
    async send(request) {
      const url = new URL(request.url);
      url.protocol = 'http:';
      url.host = 'localhost:7700';
      
      console.log('REQUEST:', request.method, url.pathname, JSON.stringify(JSON.parse(request.data || '{}'), null, 2));
      
      const response = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data
      });
      
      const text = await response.text();
      console.log('RESPONSE:', response.status, text.substring(0, 200));
      
      return {
        status: response.status,
        content: text,
        isTimedOut: false
      };
    }
  };
}

const client = algoliasearch('test-app', 'test-key', {
  requester: createLocalRequester()
});

async function test() {
  const TEST_INDEX = 'debug_' + Date.now();
  
  console.log('=== Indexing ===');
  await client.saveObjects({
    indexName: TEST_INDEX,
    objects: [
      { objectID: '1', name: 'Gaming Laptop', price: 999 }
    ]
  });
  
  // Wait for indexing
  await new Promise(resolve => setTimeout(resolve, 500));
  
  console.log('\n=== Empty Query (should work) ===');
  const emptyResult = await client.search({
    requests: [{ indexName: TEST_INDEX, query: '' }]
  });
  console.log('Hits:', emptyResult.results[0].nbHits);
  
  console.log('\n=== Text Query "laptop" (currently broken) ===');
  const textResult = await client.search({
    requests: [{ indexName: TEST_INDEX, query: 'laptop' }]
  });
  console.log('Hits:', textResult.results[0].nbHits);
  console.log('Results:', textResult.results[0].hits);
  
  console.log('\n=== Text Query "gaming" ===');
  const gamingResult = await client.search({
    requests: [{ indexName: TEST_INDEX, query: 'gaming' }]
  });
  console.log('Hits:', gamingResult.results[0].nbHits);
}

test().catch(console.error);