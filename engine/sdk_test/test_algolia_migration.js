#!/usr/bin/env node
// ============================================================================
// ALGOLIA → FLAPJACK MIGRATION TEST
// ============================================================================
//
// Proves a real customer can migrate from Algolia to Flapjack:
//   1. Creates an index on REAL Algolia (settings, synonyms, data)
//   2. Verifies it works on Algolia
//   3. Exports everything from Algolia → imports into Flapjack
//   4. Runs identical searches on both, compares results
//   5. Cleans up both sides
//
// PREREQUISITES:
//   - Algolia creds in .secret/.env.secret (ALGOLIA_APP_ID, ALGOLIA_ADMIN_KEY)
//   - Flapjack server running on localhost:7700
//
// USAGE:
//   node test_algolia_migration.js
//   node test_algolia_migration.js --verbose
//
// ============================================================================

import { algoliasearch } from 'algoliasearch';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

dotenv.config({ path: join(__dirname, '..', '.secret', '.env.secret') });

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
const ALGOLIA_APP_ID = process.env.ALGOLIA_APP_ID;
const ALGOLIA_ADMIN_KEY = process.env.ALGOLIA_ADMIN_KEY;
const FLAPJACK_URL = process.env.FLAPJACK_URL || 'http://localhost:7700';
const VERBOSE = process.argv.includes('--verbose');

if (!ALGOLIA_APP_ID || !ALGOLIA_ADMIN_KEY) {
  console.error('ERROR: Missing ALGOLIA_APP_ID or ALGOLIA_ADMIN_KEY in .secret/.env.secret');
  console.error('This test requires real Algolia credentials to prove migration works.');
  process.exit(1);
}

const INDEX_NAME = 'fj_migration_test_' + Date.now();

// ---------------------------------------------------------------------------
// Clients
// ---------------------------------------------------------------------------
function flapjackRequester(baseUrl) {
  return {
    async send(request) {
      const url = new URL(request.url);
      const target = new URL(baseUrl);
      url.protocol = target.protocol;
      url.host = target.host;
      const response = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data,
      });
      return {
        status: response.status,
        content: await response.text(),
        isTimedOut: false,
      };
    },
  };
}

const algolia = algoliasearch(ALGOLIA_APP_ID, ALGOLIA_ADMIN_KEY);
const flapjack = algoliasearch('migration-test', 'test-key', {
  requester: flapjackRequester(FLAPJACK_URL),
});

// ---------------------------------------------------------------------------
// Logging with timestamps
// ---------------------------------------------------------------------------
const T0 = Date.now();

function ts() {
  const elapsed = (Date.now() - T0) / 1000;
  const mins = Math.floor(elapsed / 60);
  const secs = elapsed % 60;
  return `[${String(mins).padStart(2, '0')}:${secs.toFixed(1).padStart(4, '0')}]`;
}

function log(msg) { console.log(`${ts()} ${msg}`); }
function logIndent(msg) { console.log(`${ts()}   ${msg}`); }

// ---------------------------------------------------------------------------
// Test data: e-commerce products
// ---------------------------------------------------------------------------
const PRODUCTS = [
  { objectID: 'p01', name: 'MacBook Pro 16"', description: 'Apple M3 Max chip laptop', brand: 'Apple', category: 'Laptops', price: 3499, rating: 4.8, inStock: true, tags: ['laptop', 'professional'] },
  { objectID: 'p02', name: 'ThinkPad X1 Carbon', description: 'Lightweight business laptop', brand: 'Lenovo', category: 'Laptops', price: 1849, rating: 4.6, inStock: true, tags: ['laptop', 'business'] },
  { objectID: 'p03', name: 'Dell XPS 15', description: 'Creative laptop with OLED display', brand: 'Dell', category: 'Laptops', price: 2499, rating: 4.5, inStock: true, tags: ['laptop', 'creative'] },
  { objectID: 'p04', name: 'iPad Pro 12.9"', description: 'M2 chip tablet by Apple', brand: 'Apple', category: 'Tablets', price: 1099, rating: 4.7, inStock: true, tags: ['tablet', 'professional'] },
  { objectID: 'p05', name: 'Galaxy Tab S9', description: 'Samsung premium Android tablet', brand: 'Samsung', category: 'Tablets', price: 1199, rating: 4.4, inStock: false, tags: ['tablet', 'android'] },
  { objectID: 'p06', name: 'Sony WH-1000XM5', description: 'Wireless noise canceling headphones', brand: 'Sony', category: 'Audio', price: 349, rating: 4.7, inStock: true, tags: ['headphones', 'wireless'] },
  { objectID: 'p07', name: 'AirPods Pro 2', description: 'Apple wireless earbuds with ANC', brand: 'Apple', category: 'Audio', price: 249, rating: 4.6, inStock: true, tags: ['earbuds', 'wireless'] },
  { objectID: 'p08', name: 'Samsung 990 Pro 2TB', description: 'NVMe M.2 SSD storage', brand: 'Samsung', category: 'Storage', price: 179, rating: 4.8, inStock: true, tags: ['ssd', 'storage'] },
  { objectID: 'p09', name: 'LG UltraGear 27" 4K', description: '144Hz gaming monitor', brand: 'LG', category: 'Monitors', price: 699, rating: 4.5, inStock: true, tags: ['monitor', 'gaming'] },
  { objectID: 'p10', name: 'Logitech MX Master 3S', description: 'Wireless ergonomic mouse', brand: 'Logitech', category: 'Accessories', price: 99, rating: 4.7, inStock: true, tags: ['mouse', 'wireless'] },
  { objectID: 'p11', name: 'Keychron Q1 Pro', description: 'Wireless mechanical keyboard', brand: 'Keychron', category: 'Accessories', price: 199, rating: 4.6, inStock: true, tags: ['keyboard', 'wireless'] },
  { objectID: 'p12', name: 'CalDigit TS4', description: 'Thunderbolt 4 dock with 18 ports', brand: 'CalDigit', category: 'Accessories', price: 399, rating: 4.8, inStock: false, tags: ['dock', 'thunderbolt'] },
];

const SYNONYMS = [
  { objectID: 'syn-laptop-notebook', type: 'synonym', synonyms: ['laptop', 'notebook', 'computer'] },
  { objectID: 'syn-phone-mobile', type: 'synonym', synonyms: ['headphones', 'earphones', 'earbuds'] },
  { objectID: 'syn-screen-display', type: 'synonym', synonyms: ['monitor', 'screen', 'display'] },
];

const RULES = [
  {
    objectID: 'rule-pin-macbook',
    conditions: [{ pattern: 'laptop', anchoring: 'contains' }],
    consequence: { promote: [{ objectID: 'p01', position: 0 }] },
    description: 'Pin MacBook Pro to top when searching laptop',
  },
  {
    objectID: 'rule-hide-galaxy-tab',
    conditions: [{ pattern: 'tablet', anchoring: 'contains' }],
    consequence: { hide: [{ objectID: 'p05' }] },
    description: 'Hide Galaxy Tab S9 when searching tablet',
  },
];

const SETTINGS = {
  searchableAttributes: ['name', 'description', 'brand', 'category', 'tags'],
  attributesForFaceting: ['category', 'brand', 'filterOnly(price)', 'filterOnly(inStock)'],
  customRanking: ['desc(rating)', 'asc(price)'],
};

// ---------------------------------------------------------------------------
// Searches to compare (run on both Algolia and Flapjack)
// ---------------------------------------------------------------------------
const SEARCHES = [
  { label: 'empty query + facets', query: '', params: { facets: ['category', 'brand'] } },
  { label: '"laptop" text search', query: 'laptop', params: {} },
  { label: '"notebook" (synonym)', query: 'notebook', params: {} },
  { label: '"apple tablet" multi-word', query: 'apple tablet', params: {} },
  { label: 'filter category:Laptops', query: '', params: { filters: 'category:Laptops' } },
  { label: 'filter price >= 1000', query: '', params: { filters: 'price >= 1000' } },
  { label: 'complex boolean filter', query: '', params: { filters: '(category:Laptops OR category:Tablets) AND brand:Apple' } },
  { label: '"wireless" + filter + facets', query: 'wireless', params: { filters: 'category:Accessories', facets: ['category'] } },
  { label: '"mac" prefix search', query: 'mac', params: {} },
  { label: '"laptop" rule pin (p01 first)', query: 'laptop', params: {}, ruleCheck: { firstHit: 'p01' } },
  { label: '"tablet" rule hide (no p05)', query: 'tablet', params: {}, ruleCheck: { hidden: 'p05' } },
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
async function searchOne(client, indexName, query, params) {
  const result = await client.search({
    requests: [{ indexName, query, ...params }],
  });
  return result.results[0];
}

async function waitForAlgoliaIndexing(expectedCount, maxWait = 15000) {
  const start = Date.now();
  while (Date.now() - start < maxWait) {
    try {
      const result = await searchOne(algolia, INDEX_NAME, '', {});
      if (result.nbHits >= expectedCount) return;
    } catch (_) {}
    await new Promise(r => setTimeout(r, 500));
  }
  throw new Error(`Algolia indexing timeout: expected ${expectedCount} docs after ${maxWait}ms`);
}

async function waitForFlapjackIndexing(expectedCount, maxWait = 10000) {
  const start = Date.now();
  while (Date.now() - start < maxWait) {
    try {
      const result = await searchOne(flapjack, INDEX_NAME, '', {});
      if (result.nbHits >= expectedCount) return;
    } catch (_) {}
    await new Promise(r => setTimeout(r, 100));
  }
  throw new Error(`Flapjack indexing timeout: expected ${expectedCount} docs after ${maxWait}ms`);
}

function compareResults(algoliaResult, flapjackResult, ruleCheck) {
  const diffs = [];

  // Compare hit counts
  if (algoliaResult.nbHits !== flapjackResult.nbHits) {
    diffs.push(`nbHits: Algolia=${algoliaResult.nbHits} Flapjack=${flapjackResult.nbHits}`);
  }

  // Compare hit IDs (set equality, not order)
  const algoliaIds = new Set(algoliaResult.hits.map(h => h.objectID));
  const flapjackIds = new Set(flapjackResult.hits.map(h => h.objectID));
  const missing = [...algoliaIds].filter(id => !flapjackIds.has(id));
  const extra = [...flapjackIds].filter(id => !algoliaIds.has(id));
  if (missing.length > 0 || extra.length > 0) {
    diffs.push(`IDs differ — missing from Flapjack: [${missing.join(', ')}], extra in Flapjack: [${extra.join(', ')}]`);
  }

  // Compare facets if present
  if (algoliaResult.facets && flapjackResult.facets) {
    for (const facetName of Object.keys(algoliaResult.facets)) {
      const af = algoliaResult.facets[facetName] || {};
      const ff = flapjackResult.facets[facetName] || {};
      const allKeys = new Set([...Object.keys(af), ...Object.keys(ff)]);
      for (const key of allKeys) {
        const aCount = parseInt(af[key] || '0');
        const fCount = parseInt(ff[key] || '0');
        if (aCount !== fCount) {
          diffs.push(`facet ${facetName}.${key}: Algolia=${aCount} Flapjack=${fCount}`);
        }
      }
    }
  }

  // Check rule effects if specified
  if (ruleCheck) {
    if (ruleCheck.firstHit && flapjackResult.hits.length > 0) {
      if (flapjackResult.hits[0].objectID !== ruleCheck.firstHit) {
        diffs.push(`rule pin: expected first hit ${ruleCheck.firstHit}, got ${flapjackResult.hits[0].objectID}`);
      }
    }
    if (ruleCheck.hidden) {
      const fjIds = flapjackResult.hits.map(h => h.objectID);
      if (fjIds.includes(ruleCheck.hidden)) {
        diffs.push(`rule hide: ${ruleCheck.hidden} should be hidden but is present in results`);
      }
    }
  }

  return diffs;
}

// ---------------------------------------------------------------------------
// Phase 1: Build index on Algolia
// ---------------------------------------------------------------------------
async function phase1_buildOnAlgolia() {
  log('');
  log('-- PHASE 1: Build index on Algolia --');

  log('setSettings (searchable, facets, ranking) ...');
  await algolia.setSettings({ indexName: INDEX_NAME, indexSettings: SETTINGS });
  log('setSettings ... OK');

  log(`saveSynonyms (${SYNONYMS.length} synonyms) ...`);
  for (const syn of SYNONYMS) {
    await algolia.saveSynonym({
      indexName: INDEX_NAME,
      objectID: syn.objectID,
      synonymHit: syn,
    });
  }
  log(`saveSynonyms ... OK`);

  log(`saveRules (${RULES.length} rules) ...`);
  for (const rule of RULES) {
    await algolia.saveRule({
      indexName: INDEX_NAME,
      objectID: rule.objectID,
      rule,
    });
  }
  log(`saveRules ... OK`);

  log(`saveObjects (${PRODUCTS.length} products) ...`);
  await algolia.saveObjects({ indexName: INDEX_NAME, objects: PRODUCTS });
  log('saveObjects ... OK');

  log('Waiting for Algolia indexing ...');
  await waitForAlgoliaIndexing(PRODUCTS.length);
  log(`Indexing complete (${PRODUCTS.length} docs searchable)`);
}

// ---------------------------------------------------------------------------
// Phase 2: Verify on Algolia (capture expected results)
// ---------------------------------------------------------------------------
async function phase2_verifyAlgolia() {
  log('');
  log('-- PHASE 2: Verify Algolia index --');

  const expected = [];
  for (const s of SEARCHES) {
    const result = await searchOne(algolia, INDEX_NAME, s.query, s.params);
    expected.push(result);

    let detail = `${result.nbHits} hits`;
    if (result.facets) {
      const facetSummary = Object.entries(result.facets)
        .map(([k, v]) => `${k}: {${Object.entries(v).map(([fk, fv]) => `${fk}:${fv}`).join(', ')}}`)
        .join(', ');
      detail += `, facets: ${facetSummary}`;
    }
    log(`Search "${s.query}" (${s.label}) -> ${detail}`);
    if (VERBOSE) logIndent(`IDs: [${result.hits.map(h => h.objectID).join(', ')}]`);
  }

  // Verify synonyms work on Algolia
  const notebookResult = expected[2]; // "notebook" search
  if (notebookResult.nbHits === 0) {
    log('WARNING: "notebook" returned 0 hits on Algolia -- synonyms may not be active');
  } else {
    logIndent(`"notebook" synonym working on Algolia (${notebookResult.nbHits} hits)`);
  }

  return expected;
}

// ---------------------------------------------------------------------------
// Phase 3: Migrate Algolia -> Flapjack
// ---------------------------------------------------------------------------
async function phase3_migrate() {
  log('');
  log('-- PHASE 3: Migrate Algolia -> Flapjack --');

  // Export settings from Algolia
  log('Exporting settings from Algolia ...');
  const settings = await algolia.getSettings({ indexName: INDEX_NAME });
  log('Exported settings from Algolia');
  if (VERBOSE) logIndent(`searchableAttributes: ${JSON.stringify(settings.searchableAttributes)}`);

  // Export synonyms from Algolia
  log('Exporting synonyms from Algolia ...');
  const synResponse = await fetch(
    `https://${ALGOLIA_APP_ID}-dsn.algolia.net/1/indexes/${INDEX_NAME}/synonyms/search`,
    {
      method: 'POST',
      headers: {
        'x-algolia-api-key': ALGOLIA_ADMIN_KEY,
        'x-algolia-application-id': ALGOLIA_APP_ID,
        'content-type': 'application/json',
      },
      body: JSON.stringify({ query: '', hitsPerPage: 1000 }),
    }
  );
  const synData = await synResponse.json();
  const exportedSynonyms = synData.hits || [];
  log(`Exported ${exportedSynonyms.length} synonyms from Algolia`);
  if (VERBOSE) logIndent(`Synonyms: ${exportedSynonyms.map(s => s.objectID).join(', ')}`);

  // Export objects from Algolia (via browse)
  log('Exporting objects from Algolia (browse) ...');
  const allObjects = [];
  let page = 0;
  while (page < 100) {
    const result = await searchOne(algolia, INDEX_NAME, '', { hitsPerPage: 1000, page });
    allObjects.push(...result.hits);
    if (page >= result.nbPages - 1) break;
    page++;
  }
  log(`Exported ${allObjects.length} objects from Algolia`);

  // Import settings into Flapjack
  log('Importing settings into Flapjack ...');
  await flapjack.setSettings({
    indexName: INDEX_NAME,
    indexSettings: {
      searchableAttributes: settings.searchableAttributes,
      attributesForFaceting: settings.attributesForFaceting,
      customRanking: settings.customRanking,
    },
  });
  log('Imported settings into Flapjack ... OK');

  // Import synonyms into Flapjack
  log(`Importing ${exportedSynonyms.length} synonyms into Flapjack ...`);
  for (const syn of exportedSynonyms) {
    // Clean _highlightResult from synonym export if present
    const cleanSyn = { ...syn };
    delete cleanSyn._highlightResult;
    await flapjack.saveSynonym({
      indexName: INDEX_NAME,
      objectID: cleanSyn.objectID,
      synonymHit: cleanSyn,
    });
  }
  log(`Imported ${exportedSynonyms.length} synonyms into Flapjack ... OK`);

  // Export rules from Algolia
  log('Exporting rules from Algolia ...');
  const rulesResponse = await fetch(
    `https://${ALGOLIA_APP_ID}-dsn.algolia.net/1/indexes/${INDEX_NAME}/rules/search`,
    {
      method: 'POST',
      headers: {
        'x-algolia-api-key': ALGOLIA_ADMIN_KEY,
        'x-algolia-application-id': ALGOLIA_APP_ID,
        'content-type': 'application/json',
      },
      body: JSON.stringify({ query: '', hitsPerPage: 1000 }),
    }
  );
  const rulesData = await rulesResponse.json();
  const exportedRules = rulesData.hits || [];
  log(`Exported ${exportedRules.length} rules from Algolia`);
  if (VERBOSE) logIndent(`Rules: ${exportedRules.map(r => r.objectID).join(', ')}`);

  // Import rules into Flapjack
  log(`Importing ${exportedRules.length} rules into Flapjack ...`);
  for (const rule of exportedRules) {
    const cleanRule = { ...rule };
    delete cleanRule._highlightResult;
    await flapjack.saveRule({
      indexName: INDEX_NAME,
      objectID: cleanRule.objectID,
      rule: cleanRule,
    });
  }
  log(`Imported ${exportedRules.length} rules into Flapjack ... OK`);

  // Import objects into Flapjack
  log(`Importing ${allObjects.length} objects into Flapjack ...`);
  // Strip highlight results from exported objects
  const cleanObjects = allObjects.map(obj => {
    const clean = { ...obj };
    delete clean._highlightResult;
    delete clean._snippetResult;
    delete clean._rankingInfo;
    return clean;
  });
  await flapjack.saveObjects({ indexName: INDEX_NAME, objects: cleanObjects });
  log(`Imported ${cleanObjects.length} objects into Flapjack ... OK`);

  // Wait for Flapjack indexing
  log('Waiting for Flapjack indexing ...');
  await waitForFlapjackIndexing(PRODUCTS.length);
  log(`Indexing complete (${PRODUCTS.length} docs searchable)`);
}

// ---------------------------------------------------------------------------
// Phase 4: Compare search results
// ---------------------------------------------------------------------------
async function phase4_compare(expectedResults) {
  log('');
  log('-- PHASE 4: Compare search results (Algolia vs Flapjack) --');

  let passed = 0;
  let failed = 0;

  for (let i = 0; i < SEARCHES.length; i++) {
    const s = SEARCHES[i];
    const algoliaResult = expectedResults[i];
    const flapjackResult = await searchOne(flapjack, INDEX_NAME, s.query, s.params);

    const diffs = compareResults(algoliaResult, flapjackResult, s.ruleCheck);

    if (diffs.length === 0) {
      let detail = `nbHits ${algoliaResult.nbHits}=${flapjackResult.nbHits}`;
      detail += ` | IDs match`;
      if (algoliaResult.facets) detail += ' | facets match';
      if (s.label.includes('synonym')) detail += ' (synonyms migrated!)';
      if (s.ruleCheck) detail += ' | rules working';
      log(`PASS  "${s.query}" (${s.label}): ${detail}`);
      passed++;
    } else {
      log(`FAIL  "${s.query}" (${s.label}):`);
      for (const d of diffs) {
        logIndent(d);
      }
      if (VERBOSE) {
        logIndent(`Algolia IDs:  [${algoliaResult.hits.map(h => h.objectID).join(', ')}]`);
        logIndent(`Flapjack IDs: [${flapjackResult.hits.map(h => h.objectID).join(', ')}]`);
      }
      failed++;
    }
  }

  return { passed, failed };
}

// ---------------------------------------------------------------------------
// Phase 3b: One-click migration (POST /1/migrate-from-algolia)
// ---------------------------------------------------------------------------
const ONECLICK_INDEX = INDEX_NAME + '_oneclick';

async function phase3b_migrateOneClick() {
  log('');
  log('-- PHASE 3b: One-click migration endpoint --');

  log(`Calling POST /1/migrate-from-algolia (source: ${INDEX_NAME} -> target: ${ONECLICK_INDEX}) ...`);
  const resp = await fetch(`${FLAPJACK_URL}/1/migrate-from-algolia`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'migration-test',
    },
    body: JSON.stringify({
      appId: ALGOLIA_APP_ID,
      apiKey: ALGOLIA_ADMIN_KEY,
      sourceIndex: INDEX_NAME,
      targetIndex: ONECLICK_INDEX,
    }),
  });

  if (!resp.ok) {
    const body = await resp.text();
    throw new Error(`migrate-from-algolia returned ${resp.status}: ${body}`);
  }

  const result = await resp.json();
  log(`One-click migration complete:`);
  logIndent(`settings: ${result.settings}`);
  logIndent(`synonyms imported: ${result.synonyms.imported}`);
  logIndent(`rules imported: ${result.rules.imported}`);
  logIndent(`objects imported: ${result.objects.imported}`);

  // Verify counts
  if (result.objects.imported !== PRODUCTS.length) {
    log(`WARNING: Expected ${PRODUCTS.length} objects, got ${result.objects.imported}`);
  }
  if (result.synonyms.imported !== SYNONYMS.length) {
    log(`WARNING: Expected ${SYNONYMS.length} synonyms, got ${result.synonyms.imported}`);
  }

  // Wait for indexing to settle
  log('Waiting for Flapjack one-click index to settle ...');
  const flapjackOneClick = algoliasearch('migration-test', 'test-key', {
    requester: flapjackRequester(FLAPJACK_URL),
  });
  const start = Date.now();
  while (Date.now() - start < 10000) {
    try {
      const r = await searchOne(flapjackOneClick, ONECLICK_INDEX, '', {});
      if (r.nbHits >= PRODUCTS.length) break;
    } catch (_) {}
    await new Promise(r => setTimeout(r, 100));
  }
  log(`One-click index ready`);

  // Verify synonyms and rules were imported into the one-click index
  const synResp = await fetch(
    `${FLAPJACK_URL}/1/indexes/${ONECLICK_INDEX}/synonyms/search`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ query: '', hitsPerPage: 100 }),
    }
  );
  const synData2 = await synResp.json();
  log(`One-click index synonyms: ${synData2.nbHits} (${synData2.hits?.map(s => s.objectID).join(', ') || 'none'})`);

  const rulesResp = await fetch(
    `${FLAPJACK_URL}/1/indexes/${ONECLICK_INDEX}/rules/search`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ query: '', hitsPerPage: 100 }),
    }
  );
  const rulesData = await rulesResp.json();
  log(`One-click index rules: ${rulesData.nbHits} (${rulesData.hits?.map(r => r.objectID).join(', ') || 'none'})`);

  return flapjackOneClick;
}

// ---------------------------------------------------------------------------
// Phase 4b: Compare one-click migration results
// ---------------------------------------------------------------------------
async function phase4b_compareOneClick(expectedResults, flapjackOneClick) {
  log('');
  log('-- PHASE 4b: Compare one-click migration results --');

  let passed = 0;
  let failed = 0;

  for (let i = 0; i < SEARCHES.length; i++) {
    const s = SEARCHES[i];
    const algoliaResult = expectedResults[i];
    const flapjackResult = await searchOne(flapjackOneClick, ONECLICK_INDEX, s.query, s.params);

    const diffs = compareResults(algoliaResult, flapjackResult, s.ruleCheck);

    if (diffs.length === 0) {
      let detail = `nbHits ${algoliaResult.nbHits}=${flapjackResult.nbHits}`;
      detail += ` | IDs match`;
      if (algoliaResult.facets) detail += ' | facets match';
      if (s.label.includes('synonym')) detail += ' (synonyms migrated!)';
      if (s.ruleCheck) detail += ' | rules working';
      log(`PASS  "${s.query}" (${s.label}): ${detail}`);
      passed++;
    } else {
      log(`FAIL  "${s.query}" (${s.label}):`);
      for (const d of diffs) {
        logIndent(d);
      }
      if (VERBOSE) {
        logIndent(`Algolia IDs:  [${algoliaResult.hits.map(h => h.objectID).join(', ')}]`);
        logIndent(`Flapjack IDs: [${flapjackResult.hits.map(h => h.objectID).join(', ')}]`);
      }
      failed++;
    }
  }

  return { passed, failed };
}

// ---------------------------------------------------------------------------
// Phase 3c: Re-migration safety (overwrite protection)
// ---------------------------------------------------------------------------
async function phase3c_remigrationSafety() {
  log('');
  log('-- PHASE 3c: Re-migration safety --');

  // Try migrating to the same one-click index WITHOUT overwrite — should get 409
  log(`Re-migrating to ${ONECLICK_INDEX} without overwrite (expect 409) ...`);
  const resp1 = await fetch(`${FLAPJACK_URL}/1/migrate-from-algolia`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'migration-test',
    },
    body: JSON.stringify({
      appId: ALGOLIA_APP_ID,
      apiKey: ALGOLIA_ADMIN_KEY,
      sourceIndex: INDEX_NAME,
      targetIndex: ONECLICK_INDEX,
    }),
  });

  let passed = 0;
  let failed = 0;

  if (resp1.status === 409) {
    log(`PASS  Got 409 Conflict as expected`);
    const body = await resp1.json();
    if (VERBOSE) logIndent(`Message: ${body.message}`);
    passed++;
  } else {
    log(`FAIL  Expected 409, got ${resp1.status}`);
    failed++;
  }

  // Now re-migrate WITH overwrite: true — should succeed
  log(`Re-migrating to ${ONECLICK_INDEX} with overwrite: true (expect 200) ...`);
  const resp2 = await fetch(`${FLAPJACK_URL}/1/migrate-from-algolia`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-algolia-api-key': 'test-key',
      'x-algolia-application-id': 'migration-test',
    },
    body: JSON.stringify({
      appId: ALGOLIA_APP_ID,
      apiKey: ALGOLIA_ADMIN_KEY,
      sourceIndex: INDEX_NAME,
      targetIndex: ONECLICK_INDEX,
      overwrite: true,
    }),
  });

  if (resp2.ok) {
    const result = await resp2.json();
    log(`PASS  Overwrite succeeded (objects: ${result.objects.imported}, synonyms: ${result.synonyms.imported}, rules: ${result.rules.imported})`);
    passed++;
  } else {
    const body = await resp2.text();
    log(`FAIL  Overwrite returned ${resp2.status}: ${body}`);
    failed++;
  }

  return { passed, failed };
}

// ---------------------------------------------------------------------------
// Phase 5: Cleanup
// ---------------------------------------------------------------------------
async function phase5_cleanup() {
  log('');
  log('-- PHASE 5: Cleanup --');

  try {
    await algolia.deleteIndex({ indexName: INDEX_NAME });
    log('Deleted index on Algolia ... OK');
  } catch (e) {
    log(`Deleted index on Algolia ... FAILED (${e.message})`);
  }

  // Delete manual migration index
  try {
    const resp = await fetch(`${FLAPJACK_URL}/1/indexes/${INDEX_NAME}`, {
      method: 'DELETE',
      headers: { 'x-algolia-api-key': 'test-key', 'x-algolia-application-id': 'migration-test' },
    });
    log(`Deleted index on Flapjack (manual) ... ${resp.ok ? 'OK' : 'FAILED (' + resp.status + ')'}`);
  } catch (e) {
    log(`Deleted index on Flapjack (manual) ... FAILED (${e.message})`);
  }

  // Delete one-click migration index
  try {
    const resp = await fetch(`${FLAPJACK_URL}/1/indexes/${ONECLICK_INDEX}`, {
      method: 'DELETE',
      headers: { 'x-algolia-api-key': 'test-key', 'x-algolia-application-id': 'migration-test' },
    });
    log(`Deleted index on Flapjack (one-click) ... ${resp.ok ? 'OK' : 'FAILED (' + resp.status + ')'}`);
  } catch (e) {
    log(`Deleted index on Flapjack (one-click) ... FAILED (${e.message})`);
  }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  console.log('');
  log('=== ALGOLIA -> FLAPJACK MIGRATION TEST ===');
  log(`Algolia app: ${ALGOLIA_APP_ID} | Flapjack: ${FLAPJACK_URL}`);
  log(`Test index: ${INDEX_NAME}`);

  // Check Flapjack is up
  try {
    const health = await fetch(`${FLAPJACK_URL}/health`);
    if (!health.ok) throw new Error(`status ${health.status}`);
    log('Flapjack health: OK');
  } catch (e) {
    log(`ERROR: Cannot reach Flapjack at ${FLAPJACK_URL}`);
    log('Start the server first: cargo run (or ./scripts/dev-server.sh start)');
    process.exit(1);
  }

  // Check Algolia is reachable
  try {
    const resp = await fetch(`https://${ALGOLIA_APP_ID}-dsn.algolia.net/1/indexes`, {
      headers: {
        'x-algolia-api-key': ALGOLIA_ADMIN_KEY,
        'x-algolia-application-id': ALGOLIA_APP_ID,
      },
    });
    if (!resp.ok) throw new Error(`status ${resp.status}`);
    log('Algolia API: OK');
  } catch (e) {
    log(`ERROR: Cannot reach Algolia API (${e.message})`);
    process.exit(1);
  }

  try {
    await phase1_buildOnAlgolia();
    const expectedResults = await phase2_verifyAlgolia();

    // Manual migration (Phase 3 + 4)
    await phase3_migrate();
    const manual = await phase4_compare(expectedResults);

    // One-click migration (Phase 3b + 4b)
    const flapjackOneClick = await phase3b_migrateOneClick();
    const oneclick = await phase4b_compareOneClick(expectedResults, flapjackOneClick);

    // Re-migration safety (Phase 3c)
    const remigration = await phase3c_remigrationSafety();

    const totalPassed = manual.passed + oneclick.passed + remigration.passed;
    const totalFailed = manual.failed + oneclick.failed + remigration.failed;

    log('');
    log('=== RESULTS ===');
    log(`Manual migration:    ${manual.passed}/${manual.passed + manual.failed} searches match`);
    log(`One-click migration: ${oneclick.passed}/${oneclick.passed + oneclick.failed} searches match`);
    log(`Re-migration safety: ${remigration.passed}/${remigration.passed + remigration.failed} checks pass`);
    log(`Total: ${totalPassed}/${totalPassed + totalFailed} tests pass`);

    if (totalFailed > 0) {
      log(`Migration: ${totalFailed} FAILURES`);
    } else {
      log('Migration: ALL VERIFIED (manual, one-click, and re-migration safety)');
    }

    await phase5_cleanup();

    log('');
    process.exit(totalFailed > 0 ? 1 : 0);
  } catch (e) {
    log(`FATAL: ${e.message}`);
    if (VERBOSE) console.error(e.stack);
    log('Attempting cleanup ...');
    await phase5_cleanup();
    process.exit(1);
  }
}

main();
