export async function waitForFlapjackIndexing(client, indexName, expectedCount, maxWaitMs = 5000) {
  const start = Date.now();
  const pollInterval = 50;
  
  while (Date.now() - start < maxWaitMs) {
    try {
      const result = await client.search({ 
        requests: [{ indexName, query: '', hitsPerPage: 0 }] 
      });
      
      const currentCount = result.results[0].nbHits;
      console.log(`  [Polling: ${currentCount}/${expectedCount} docs, elapsed: ${Date.now() - start}ms]`);
      
      if (currentCount >= expectedCount) {
        return;
      }
    } catch (e) {
      console.log(`  [Poll error: ${e.message}]`);
    }
    
    await new Promise(r => setTimeout(r, pollInterval));
  }
  
  throw new Error(`Flapjack indexing timeout after ${maxWaitMs}ms: expected ${expectedCount} documents in ${indexName}`);
}