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
    name: 'searchableAttributes persistence',
    category: 'settings',
    async run(clients, fixtures) {
      const indexName = testIndexName('settings', 1);
      
      await Promise.all([
        clients.algolia.setSettings({
          indexName,
          indexSettings: {
            searchableAttributes: ['name', 'description']
          }
        }),
        clients.flapjack.setSettings({
          indexName,
          indexSettings: {
            searchableAttributes: ['name', 'description']
          }
        })
      ]);
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 20) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 20) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 20);
      
      const [algoliaSettings, flapjackSettings] = await Promise.all([
        clients.algolia.getSettings({ indexName }),
        clients.flapjack.getSettings({ indexName })
      ]);
      
      return {
        algolia: algoliaSettings,
        flapjack: flapjackSettings
      };
    },
    validate(algolia, flapjack, verbose) {
      const diffs = deepCompare(algolia, flapjack, { ignore: ['processingTimeMS', 'taskID', 'cursor', 'serverTimeMS', 'processingTimingsMS', 'params', 'minWordSizefor1Typo', 'minWordSizefor2Typos', 'hitsPerPage', 'maxValuesPerFacet', 'ranking', 'customRanking', 'separatorsToIndex', 'removeWordsIfNoResults', 'queryType', 'highlightPreTag', 'highlightPostTag', 'snippetEllipsisText', 'alternativesAsExact'] });
      if (diffs.length > 0) {
        throw { diffs };
      }
    }
  },
  
  {
    name: 'Custom ranking formula',
    category: 'settings',
    async run(clients, fixtures) {
      const indexName = testIndexName('settings', 2);
      
      await clients.flapjack.setSettings({
        indexName,
        indexSettings: {
          customRanking: ['desc(rating)', 'asc(price)']
        }
      });
      
      const [[algoliaTask]] = await Promise.all([
        clients.algolia.saveObjects({ indexName, objects: fixtures.slice(0, 20) }),
        clients.flapjack.saveObjects({ indexName, objects: fixtures.slice(0, 20) })
      ]);
      
      await clients.algolia.waitForTask({ indexName, taskID: algoliaTask.taskID });
      await waitForFlapjackIndexing(clients.flapjack, indexName, 20);
      
      const [algolia, flapjack] = await Promise.all([
        clients.algolia.search({ requests: [{ indexName, query: '' }] }),
        clients.flapjack.search({ requests: [{ indexName, query: '' }] })
      ]);
      
      return {
        algolia: algolia.results[0],
        flapjack: flapjack.results[0]
      };
    },
    validate(algolia, flapjack, verbose) {
      const algoliaIds = algolia.hits.map(h => h.objectID);
      const flapjackIds = flapjack.hits.map(h => h.objectID);
      
      if (JSON.stringify(algoliaIds) !== JSON.stringify(flapjackIds)) {
        throw { 
          diffs: [{ 
            path: 'hits[].objectID order',
            issue: 'ranking_mismatch',
            algolia: algoliaIds.join(','),
            flapjack: flapjackIds.join(',')
          }]
        };
      }
    }
  }
];