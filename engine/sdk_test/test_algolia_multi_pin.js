import { algoliasearch } from 'algoliasearch';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

dotenv.config({ path: join(__dirname, '..', '.secret', '.env.secret') });

const ALGOLIA_APP_ID = process.env.ALGOLIA_APP_ID;
const ALGOLIA_API_KEY = process.env.ALGOLIA_ADMIN_KEY;

if (!ALGOLIA_APP_ID || !ALGOLIA_API_KEY) {
  console.error('Missing Algolia credentials');
  process.exit(1);
}

const algoliaClient = algoliasearch(ALGOLIA_APP_ID, ALGOLIA_API_KEY);

async function testMultiPin() {
  const testIndex = 'multi_pin_test_' + Date.now();
  
  console.log('Testing Algolia behavior: Two rules pin different items to position 0\n');
  
  await algoliaClient.saveObjects({
    indexName: testIndex,
    objects: [
      { objectID: '1', name: 'Gaming Laptop', price: 999 },
      { objectID: '2', name: 'Office Laptop', price: 599 },
      { objectID: '3', name: 'Budget Laptop', price: 399 },
      { objectID: '4', name: 'Laptop Stand', price: 49 },
    ]
  });
  
  await new Promise(resolve => setTimeout(resolve, 2000));
  
  console.log('Natural order:');
  const natural = await algoliaClient.search({
    requests: [{ indexName: testIndex, query: 'laptop' }]
  });
  console.log('  ', natural.results[0].hits.map(h => h.objectID).join(', '));
  
  console.log('\nSaving rule pin-a (objectID 1 → position 0)');
  await algoliaClient.saveRule({
    indexName: testIndex,
    objectID: 'pin-a',
    rule: {
      objectID: 'pin-a',
      conditions: [{ pattern: 'laptop', anchoring: 'contains' }],
      consequence: {
        promote: [{ objectID: '1', position: 0 }]
      }
    }
  });
  
  await new Promise(resolve => setTimeout(resolve, 2000));
  
  console.log('\nSaving rule pin-b (objectID 2 → position 0)');
  await algoliaClient.saveRule({
    indexName: testIndex,
    objectID: 'pin-b',
    rule: {
      objectID: 'pin-b',
      conditions: [{ pattern: 'laptop', anchoring: 'contains' }],
      consequence: {
        promote: [{ objectID: '2', position: 0 }]
      }
    }
  });
  
  await new Promise(resolve => setTimeout(resolve, 2000));
  
  console.log('\nAlgolia result (both rules active):');
  const result = await algoliaClient.search({
    requests: [{ indexName: testIndex, query: 'laptop' }]
  });
  const order = result.results[0].hits.map(h => h.objectID);
  console.log('  ', order.join(', '));
  
  console.log('\nDetailed positions:');
  result.results[0].hits.forEach((h, i) => {
    console.log(`  [${i}] ${h.objectID}`);
  });
  
  console.log('\nInterpretation:');
  if (order[0] === '2' && order[1] === '1') {
    console.log('  ✓ Last-saved rule wins position 0, previous pushed to position 1');
  } else if (order[0] === '1' && order[1] === '2') {
    console.log('  ✓ First-saved rule wins position 0, later pushed to position 1');
  } else if (order.slice(0, 2).includes('1') && order.slice(0, 2).includes('2')) {
    console.log('  ✓ Both in top 2, insertion order:', order.slice(0, 2).join(', '));
  } else {
    console.log('  ✗ Unexpected behavior:', order.join(', '));
  }
  
  await algoliaClient.deleteIndex({ indexName: testIndex });
}

testMultiPin().catch(console.error);