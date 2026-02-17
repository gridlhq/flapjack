import { deepCompare } from '../lib/deep-compare.js';
import { waitForFlapjackIndexing } from '../lib/wait-for-indexing.js';
import crypto from 'crypto';

//file edit to change hash? v2

function testIndexName(suite, caseNum) {
  const hash = crypto.createHash('md5')
    .update(`${suite}_${caseNum}`)
    .digest('hex')
    .slice(0, 8);
  return `test_${hash}`;
}

export default [
  {
    name: 'Empty query returns _highlightResult',
    category: 'highlighting',
    async run(clients, fixtures) {
      console.log('  [fixtures available:', !!fixtures, 'count:', fixtures?.length, ']');
      console.log('  [clients.algolia type:', typeof clients.algolia, 'has saveObjects:', typeof clients.algolia?.saveObjects, ']');
      const indexName = testIndexName('highlighting', 1);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name', 'description', 'brand', 'category'],
          attributesForFaceting: ['category', 'brand']
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 10) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 10) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 10);
      
      let algolia, flapjack;
      
      try {
        algolia = await clients.algolia.search({ requests: [{ indexName, query: '' }] });
        console.log('  [algolia search succeeded]');
      } catch (e) {
        console.log('  [algolia search FAILED:', e.message, ']');
        throw e;
      }
      
      try {
        flapjack = await clients.flapjack.search({ requests: [{ indexName, query: '' }] });
        console.log('  [flapjack search succeeded]');
        console.log('  [flapjack result structure]', JSON.stringify(flapjack, null, 2).slice(0, 500));
      } catch (e) {
        console.log('  [flapjack search FAILED:', e.message, ']');
        console.log('  [flapjack error details]', JSON.stringify(e, null, 2));
        throw e;
      }
      
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
    name: 'Query with matches',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 2);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 10) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 10) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 10);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'mascara' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'mascara' }] })
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
    name: 'Nested field highlighting',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 3);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name', 'description', 'brand', 'product.name', 'product.brand'],
          attributesForFaceting: ['category', 'brand']
        }
      });
      
      const nested = fixtures.slice(0, 5).map(f => ({
        ...f,
        product: { name: f.name, brand: f.brand }
      }));
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: nested }),
        clients.flapjack.saveObjects({ indexName, objects: nested })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 5);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'essence' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'essence' }] })
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
    name: 'Typo tolerance - fuzzy match highlighting',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 4);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name', 'description'],
          typoTolerance: true
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 10) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 10) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 10);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'mascra' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'mascra' }] })
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
    name: 'Typo tolerance - transposition highlighting',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 6);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name', 'brand', 'description']
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 50) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 50) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 50);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'appel' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'appel' }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      if (flapjack.nbHits === 0) {
        throw { diffs: [{ path: 'nbHits', issue: 'no_results', flapjack: 0 }] };
      }
      
      const flapjackHighlighted = flapjack.hits.filter(h => {
        const hl = h._highlightResult || {};
        return Object.values(hl).some(f => f.matchLevel === 'full' || f.matchLevel === 'partial');
      });
      
      if (flapjackHighlighted.length === 0) {
        throw { diffs: [{ path: 'highlighting', issue: 'no_highlights', message: 'No hits have matchLevel full/partial' }] };
      }
      
      const hasEmTag = flapjack.hits.some(h => {
        const hl = h._highlightResult || {};
        return Object.values(hl).some(f => f.value && f.value.includes('<em>'));
      });
      
      if (!hasEmTag) {
        throw { diffs: [{ path: 'highlighting', issue: 'no_em_tags', message: 'No <em> tags found in highlight values' }] };
      }
    }
  },
  
  {
    name: 'Typo tolerance - first char error highlighting',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 7);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name', 'brand', 'description']
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 50) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 50) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 50);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'lsha' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'lsha' }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      if (flapjack.nbHits === 0) {
        throw { diffs: [{ path: 'nbHits', issue: 'no_results', flapjack: 0 }] };
      }
      
      const hasEmTag = flapjack.hits.some(h => {
        const hl = h._highlightResult || {};
        return Object.values(hl).some(f => f.value && f.value.includes('<em>'));
      });
      
      if (!hasEmTag) {
        throw { diffs: [{ path: 'highlighting', issue: 'no_em_tags', message: 'lsha should highlight sha in shades' }] };
      }
    }
  },
  
  {
    name: 'Trailing space does not affect fuzzy results for long queries',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 8);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          searchableAttributes: ['name', 'brand', 'description']
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 50) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 50) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 50);
      
      const [withoutSpace, withSpace] = await Promise.all([
        clients.flapjack.search({ requests: [{ indexName, query: 'fashoin' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'fashoin ' }] })
      ]);
      
      return {
        algolia: { nbHits: withoutSpace.results[0].nbHits },
        flapjack: { nbHits: withSpace.results[0].nbHits }
      };
    },
    validate(algolia, flapjack, verbose) {
      if (algolia.nbHits !== flapjack.nbHits) {
        throw { diffs: [{ 
          path: 'nbHits', 
          issue: 'trailing_space_mismatch', 
          without_space: algolia.nbHits, 
          with_space: flapjack.nbHits,
          message: 'Trailing space should not change hit count for fuzzy queries >= 4 chars'
        }] };
      }
    }
  },
  
  {
    name: 'Multi-word spans',
    category: 'highlighting',
    async run(clients, fixtures) {
      const indexName = testIndexName('highlighting', 5);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 10) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 10) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 10);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: 'essence mascara' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: 'essence mascara' }] })
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
  }];