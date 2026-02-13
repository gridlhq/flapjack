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
