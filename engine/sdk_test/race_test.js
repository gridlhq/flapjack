import { createFlapjackClient } from './lib/flapjack-client.js';

const client = createFlapjackClient();

const INDEX = 'race_' + Date.now();

await client.saveObjects({
  indexName: INDEX,
  objects: [{ objectID: '1', name: 'Test' }]
});

const result = await client.search({
  requests: [{ indexName: INDEX, query: 'test' }]
});

console.log('Hits:', result.results[0].nbHits);
process.exit(result.results[0].nbHits > 0 ? 0 : 1);
