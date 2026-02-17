import dotenvx from '@dotenvx/dotenvx';
dotenvx.config({ path: '../.secret/.env.secret' });

import algoliasearch from 'algoliasearch';
import { FlapjackClient } from './sdk/flapjack-sdk.js';

const algolia = algoliasearch(process.env.ALGOLIA_APP_ID, process.env.ALGOLIA_API_KEY);
const flapjack = new FlapjackClient(process.env.FLAPJACK_BASE_URL, process.env.FLAPJACK_API_KEY);

const indexName = 'test_debug_highlight';

// Clean up first
try { await flapjack.deleteIndex({ indexName }); } catch (e) {}

const doc = {
  objectID: '1',
  name: 'Essence Mascara Lash Princess',
  brand: 'Essence',
  price: 9.99,
  category: 'beauty',
  description: 'The Essence Mascara Lash Princess is a popular mascara.',
  rating: 2.56,
  tags: ['beauty', 'mascara']
};

await Promise.all([
  algolia.saveObjects({ indexName, objects: [doc] }),
  flapjack.saveObjects({ indexName, objects: [doc] })
]);

await new Promise(r => setTimeout(r, 1000));

const [algoliaRes, flapjackRes] = await Promise.all([
  algolia.search({ requests: [{ indexName, query: 'essence mascara' }] }),
  flapjack.search({ requests: [{ indexName, query: 'essence mascara' }] })
]);

console.log('\n=== ALGOLIA brand ===');
console.log(JSON.stringify(algoliaRes.results[0].hits[0]._highlightResult.brand, null, 2));

console.log('\n=== FLAPJACK brand ===');
console.log(JSON.stringify(flapjackRes.results[0].hits[0]._highlightResult.brand, null, 2));

console.log('\n=== ALGOLIA tags[1] ===');
console.log(JSON.stringify(algoliaRes.results[0].hits[0]._highlightResult.tags[1], null, 2));

console.log('\n=== FLAPJACK tags[1] ===');
console.log(JSON.stringify(flapjackRes.results[0].hits[0]._highlightResult.tags[1], null, 2));

await flapjack.deleteIndex({ indexName });
