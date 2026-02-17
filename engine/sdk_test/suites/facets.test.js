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
    name: 'Facet count accuracy',
    category: 'facets',
    async run(clients, fixtures) {
      const indexName = testIndexName('facets', 1);
      
      await Promise.all([
        clients.algolia.setSettings({
          indexName,
          indexSettings: { attributesForFaceting: ['category'] }
        }),
        clients.flapjack.setSettings({
          indexName,
          indexSettings: { attributesForFaceting: ['category'] }
        })
      ]);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 30) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 30) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 30);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '', facets: ['category'] }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '', facets: ['category'] }] })
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
    name: 'Hierarchical facets',
    category: 'facets',
    async run(clients, fixtures) {
      const indexName = testIndexName('facets', 2);
      
      const hierarchicalDocs = [
        { objectID: '1', name: 'Laptop', categories: { lvl0: 'Electronics', lvl1: 'Electronics > Computers' } },
        { objectID: '2', name: 'Phone', categories: { lvl0: 'Electronics', lvl1: 'Electronics > Phones' } },
        { objectID: '3', name: 'Novel', categories: { lvl0: 'Books', lvl1: 'Books > Fiction' } }
      ];
      
      await Promise.all([
        clients.algolia.setSettings({
          indexName,
          indexSettings: { attributesForFaceting: ['categories.lvl0', 'categories.lvl1'] }
        }),
        clients.flapjack.setSettings({
          indexName,
          indexSettings: { attributesForFaceting: ['categories.lvl0', 'categories.lvl1'] }
        })
      ]);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: hierarchicalDocs }),
        clients.flapjack.saveObjects({ indexName, objects: hierarchicalDocs })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 3);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '', facets: ['categories.lvl0'] }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '', facets: ['categories.lvl0'] }] })
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
    name: 'Facets with active filters',
    category: 'facets',
    async run(clients, fixtures) {
      const indexName = testIndexName('facets', 3);
      
      await Promise.all([
        clients.algolia.setSettings({
          indexName,
          indexSettings: {
            attributesForFaceting: ['category', 'brand']
          }
        }),
        clients.flapjack.setSettings({
          indexName,
          indexSettings: {
            attributesForFaceting: ['category', 'brand']
          }
        })
      ]);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 30) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 30) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 30);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '', filters: 'category:beauty', facets: ['brand'] }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '', filters: 'category:beauty', facets: ['brand'] }] })
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