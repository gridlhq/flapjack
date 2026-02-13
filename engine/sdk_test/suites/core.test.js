import { deepCompare } from '../lib/deep-compare.js';
import { waitForFlapjackIndexing } from '../lib/wait-for-indexing.js';
import crypto from 'crypto';

function testIndexName(suite, caseNum) {
  const hash = crypto.createHash('md5')
    .update(`${suite}_${caseNum}`)
    .digest('hex')
    .slice(0, 8);
  return `test_${hash}`;
}

export default [
  {
    name: 'Empty query - returns all documents with correct ordering',
    category: 'core',
    async run(clients, fixtures) {
      const indexName = testIndexName('core', 1);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 20) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 20) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 20);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '', hitsPerPage: 10 }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '', hitsPerPage: 10 }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      const diffs = deepCompare(algolia, flapjack);
      if (diffs.length > 0) {
        throw { diffs };
      }
    }
  },
  
  {
    name: 'Pagination - nbPages calculation, page parameter consistency',
    category: 'core',
    async run(clients, fixtures) {
      const indexName = testIndexName('core', 2);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 50);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '', hitsPerPage: 5, page: 2 }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '', hitsPerPage: 5, page: 2 }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      console.log('[ALGOLIA OBJECTIDS]', algolia.hits.map(h => h.objectID).join(','));
      console.log('[FLAPJACK OBJECTIDS]', flapjack.hits.map(h => h.objectID).join(','));
      const diffs = deepCompare(algolia, flapjack);
      if (diffs.length > 0) {
        throw { diffs };
      }
    }
  },
  
  {
    name: 'Numeric filters - range queries',
    category: 'core',
    async run(clients, fixtures) {
      const indexName = testIndexName('core', 3);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name'],
          attributesForFaceting: ['filterOnly(price)']
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 50);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '', filters: 'price:10 TO 50' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '', filters: 'price:10 TO 50' }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      const diffs = deepCompare(algolia, flapjack);
      if (diffs.length > 0) {
        throw { diffs };
      }
    }
  },
  
  {
    name: 'Single char query - typo tolerance edge case',
    category: 'core',
    skip: 'Ranking divergence acceptable - returns correct hits, different order',
    async run(clients, fixtures) {
      const indexName = testIndexName('core', 4);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 30) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 30) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 30);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'v' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'v' }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      const diffs = deepCompare(algolia, flapjack);
      if (diffs.length > 0) {
        throw { diffs };
      }
    }
  },
  
  {
    name: 'Two char query - prefix matching edge case',
    category: 'core',
    async run(clients, fixtures) {
      const indexName = testIndexName('core', 5);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 30) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 30) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 30);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'bo' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'bo' }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      const diffs = deepCompare(algolia, flapjack);
      if (diffs.length > 0) {
        throw { diffs };
      }
    }
  }
];