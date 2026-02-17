import crypto from 'crypto';
import { formatDiffs } from './deep-compare.js';

export class TestRunner {
  constructor(clients, cache, opts = {}) {
    this.clients = clients;
    this.cache = cache;
    this.stopOnFail = opts.stopOnFail || false;
    this.useCache = opts.useCache !== false;
    this.verbose = opts.verbose || false;
    this.runId = crypto.randomBytes(3).toString('hex');
  }
  
  async run(suites, caseFilter = null) {
    const suiteStartTime = Date.now();
    const results = {
      passed: 0,
      failed: 0,
      details: [],
      suiteTimes: []
    };
    
    for (const suite of suites) {
      let suiteLabel = suite.name;
      if (caseFilter) {
        const activeCases = [];
        for (let i = 0; i < suite.cases.length; i++) {
          if (caseFilter(suite.name, i + 1)) activeCases.push(i + 1);
        }
        if (activeCases.length < suite.cases.length) {
          suiteLabel += ':' + (activeCases.length === 1 ? activeCases[0] : `${activeCases[0]}-${activeCases[activeCases.length - 1]}`);
        }
      }
      
      const suiteStart = Date.now();
      const hash = await this.cache._computeHash(suite);
      const cacheDir = `.cache/${suite.name}/${hash}`;
      const cacheMode = this.useCache ? `cache: ${cacheDir}` : 'hitting Algolia API';
      console.log(`\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`);
      console.log(`Suite: ${suiteLabel} (${cacheMode})`);
      console.log(`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n`);
      
      for (let i = 0; i < suite.cases.length; i++) {
        const caseStart = Date.now();
        const testCase = suite.cases[i];
        const caseNum = i + 1;
        
        if (caseFilter && !caseFilter(suite.name, caseNum)) continue;
        
        testCase.runId = this.runId;
        
        console.log(`[${caseNum}/${suite.cases.length}] ${testCase.name}`);
        
        if (testCase.skip) {
          console.log(`  ⏭️  SKIP: ${testCase.skip}\n`);
          continue;
        }
        
        try {
          let algolia = null;
          let fromCache = false;
          
          if (this.useCache) {
            algolia = await this.cache.get(suite, caseNum);
            fromCache = !!algolia;
          }
          
          const responses = await testCase.run(this.clients, this.fixtures);
          const flapjack = responses.flapjack;
          
          if (!algolia) {
            algolia = responses.algolia;
            if (this.useCache) {
              await this.cache.set(suite, caseNum, algolia);
            }
          }
          
          if (this.verbose) {
            if (fromCache) {
              console.log('  [Algolia: cached | Flapjack: fresh]');
            } else {
              console.log('  [Algolia: API hit | Flapjack: fresh]');
            }
          }
          
          if (testCase.validate) {
            testCase.validate(algolia, flapjack, this.verbose);
          }
          
          const caseTime = Date.now() - caseStart;
          console.log(`  ✅ PASS (${caseTime}ms)\n`);
          results.passed++;
          results.details.push({ suite: suite.name, case: caseNum, name: testCase.name, status: 'pass' });
          
        } catch (e) {
          if (e.diffs) {
            const msg = this.verbose ? formatDiffs(e.diffs, true) : `${e.diffs.length} divergence(s)`;
            console.log(`  ❌ FAIL: ${msg}\n`);
            results.failed++;
            results.details.push({ suite: suite.name, case: caseNum, name: testCase.name, status: 'fail', error: msg });
          } else {
            console.log(`  ❌ FAIL: ${e.message}\n`);
            results.failed++;
            results.details.push({ suite: suite.name, case: caseNum, name: testCase.name, status: 'fail', error: e.message });
          }
          
          if (this.stopOnFail) {
            console.log('\n⚠️  Stopping on first failure (--stop-on-fail)\n');
            break;
          }
        }
      }
      
      const suiteTime = Date.now() - suiteStart;
      console.log(`Suite completed in ${suiteTime}ms\n`);
      results.suiteTimes.push({ name: suiteLabel, time: suiteTime });
      
      if (this.stopOnFail && results.failed > 0) break;
    }
    
    const totalTime = Date.now() - suiteStartTime;
    console.log(`\nTotal runtime: ${totalTime}ms`);
    
    await this.cleanup();
    
    if (results.details.length > 0) {
      console.log('\n═══════════════════════════════════════');
      console.log('DETAILS');
      console.log('═══════════════════════════════════════');
      for (const d of results.details) {
        const icon = d.status === 'pass' ? '✅' : '❌';
        console.log(`${icon} [${d.suite}:${d.case}] ${d.name}`);
      }
    }
    
    return results;
  }
  
  async cleanup() {
    console.log('\nCleaning up test indices...');
    
    try {
      const { items } = await this.clients.algolia.listIndices();
      const testIndices = items.filter(i => i.name.startsWith(`test_`));
      
      for (const idx of testIndices) {
        try {
          await Promise.all([
            this.clients.algolia.deleteIndex({ indexName: idx.name }),
            this.clients.flapjack.deleteIndex({ indexName: idx.name })
          ]);
          console.log(`  Deleted: ${idx.name}`);
        } catch (e) {
          console.log(`  Failed to delete ${idx.name}: ${e.message}`);
        }
      }
    } catch (e) {
      console.log(`  Cleanup failed: ${e.message}`);
    }
  }
}