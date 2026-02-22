import { createFlapjackClient } from './lib/flapjack-client.js';

const client = createFlapjackClient({ debug: true });

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