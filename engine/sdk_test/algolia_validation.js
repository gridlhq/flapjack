import { algoliasearch } from 'algoliasearch';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { execSync } from 'child_process';
import { CacheManager } from './lib/cache-manager.js';
import { TestRunner } from './lib/test-runner.js';
import { loadFixtures } from './lib/fixtures.js';
import highlightingTests from './suites/highlighting.test.js';
import coreTests from './suites/core.test.js';
import facetsTests from './suites/facets.test.js';
import settingsTests from './suites/settings.test.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

dotenv.config({ path: join(__dirname, '..', '.secret', '.env.secret') });

const ALGOLIA_APP_ID = process.env.ALGOLIA_APP_ID;
const ALGOLIA_API_KEY = process.env.ALGOLIA_ADMIN_KEY;

const args = process.argv.slice(2);

if (args.includes('--man') || args.includes('--help') || args.includes('-h')) {
  console.log(`
USAGE:
  node algolia_validation.js [suite[:case]] [flags]

EXAMPLES:
  node algolia_validation.js                    # Run all tests with cache
  node algolia_validation.js highlighting       # Run all highlighting tests
  node algolia_validation.js highlighting:2     # Run only test case 2
  node algolia_validation.js highlighting:1-3   # Run test cases 1-3
  node algolia_validation.js --no-cache         # Skip cache, hit Algolia API
  node algolia_validation.js --clear-cache      # Delete all cached responses
  node algolia_validation.js --verbose          # Show detailed diff output
  node algolia_validation.js --cleanup          # Delete test indices and exit

FLAGS:
  --man, --help, -h    Show this help
  --clear-cache        Delete cache before running tests
  --no-cache           Don't read OR write cache (always hit Algolia API)
  --stop-on-fail       Stop immediately on first test failure
  --cleanup            Delete all test_* indices (doesn't run tests)
  --verbose            Show detailed diffs (not just counts)

CACHE BEHAVIOR:
  Default: Read cache if exists, write if missing, hit Algolia API only when needed
  --clear-cache: Delete cache first, then run tests normally (writes new cache)
  --no-cache: Ignore cache completely (no reads, no writes, always fresh API hits)
  --clear-cache --no-cache: Delete cache, run tests, don't save results
  
  Location: .cache/{suite}/{hash}/case_N_algolia.json
  Hash: Auto-invalidates when test code or fixtures change

SUITE FILTERS:
  highlighting         Run all cases in suite
  highlighting:2       Run only case 2
  highlighting:1-3     Run cases 1-3
  
Run with no args to see available suites.
`);
  process.exit(0);
}

const flags = {
  clearCache: args.includes('--clear-cache'),
  noCache: args.includes('--no-cache'),
  stopOnFail: args.includes('--stop-on-fail'),
  cleanup: args.includes('--cleanup'),
  verbose: args.includes('--verbose')
};

const suiteFilters = args.filter(a => !a.startsWith('--'));

const algoliaClient = algoliasearch(ALGOLIA_APP_ID, ALGOLIA_API_KEY);
const flapjackClient = algoliasearch('test-app', 'test-key', {
  requester: {
    async send(request) {
      const url = new URL(request.url);
      url.protocol = 'http:';
      url.host = 'localhost:7700';
      
      if (flags.verbose && request.data) {
        console.log('[FLAPJACK BODY]', request.data.slice(0, 300));
      }
      
      const response = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data
      });
      
      const content = await response.text();
      
      if (flags.verbose) {
        console.log('[FLAPJACK RESPONSE]', response.status, content.slice(0, 200));
      }
      
      if (response.status >= 400 && flags.verbose) {
        console.log('[FLAPJACK FULL ERROR]', content);
      }
      
      return {
        status: response.status,
        content: content,
        isTimedOut: false,
        headers: Object.fromEntries(response.headers.entries())
      };
    }
  }
});

function createFlapjackClient(verbose) {
  return algoliasearch('test-app', 'test-key', {
    requester: {
      async send(request) {
        const url = new URL(request.url);
        url.protocol = 'http:';
        url.host = 'localhost:7700';
        
        if (verbose && request.data) {
          console.log('[FLAPJACK BODY]', request.data.slice(0, 300));
        }
        
        const response = await fetch(url.toString(), {
          method: request.method,
          headers: request.headers,
          body: request.data
        });
        
        const content = await response.text();
        
        if (verbose) {
          console.log('[FLAPJACK RESPONSE]', response.status, content.slice(0, 200));
        }
        
        if (response.status >= 400 && verbose) {
          console.log('[FLAPJACK FULL ERROR]', content);
        }
        
        return {
          status: response.status,
          content: content,
          isTimedOut: false,
          headers: Object.fromEntries(response.headers.entries())
        };
      }
    }
  });
}

function parseCaseFilter(str) {
  const [suite, range] = str.split(':');
  if (!range) return { suite, cases: null };
  
  const [start, end] = range.split('-').map(Number);
  const cases = [];
  for (let i = start; i <= (end || start); i++) cases.push(i);
  
  return { suite, cases };
}

const filters = suiteFilters.map(parseCaseFilter);

async function ensureServer() {
  try {
    const res = await fetch('http://localhost:7700/health');
    if (res.ok) return;
  } catch {}

  console.log('Release server not running. Starting it...');
  const repoRoot = join(__dirname, '..');
  execSync('./s/dev-server.sh --release restart', { cwd: repoRoot, stdio: 'inherit' });
}

async function main() {
  const cache = new CacheManager();

  if (args.length === 0) {
    console.log('\nNo arguments provided. Run with --man for help.\n');
  }

  await ensureServer();
  
  if (flags.cleanup) {
    const flapjackClient = createFlapjackClient(flags.verbose);
    const runner = new TestRunner({ algolia: algoliaClient, flapjack: flapjackClient }, cache);
    await runner.cleanup();
    return;
  }
  
  const flapjackClient = createFlapjackClient(flags.verbose);
  
  if (flags.clearCache) {
    await cache.clear();
  }
  
  const fixtures = await loadFixtures();
  
  const allSuites = [
    { name: 'core', cases: coreTests },
    { name: 'highlighting', cases: highlightingTests },
    { name: 'facets', cases: facetsTests },
    { name: 'settings', cases: settingsTests }
  ];
  
  let suites = allSuites;
  if (filters.length > 0) {
    suites = suites.filter(s => filters.some(f => f.suite === s.name));
  }
  
  const caseFilter = filters.length === 0 ? null : (suite, caseNum) => {
    const filter = filters.find(f => f.suite === suite);
    return filter && (!filter.cases || filter.cases.includes(caseNum));
  };
  
  const runner = new TestRunner(
    { algolia: algoliaClient, flapjack: flapjackClient },
    cache,
    { stopOnFail: flags.stopOnFail, useCache: !flags.noCache, verbose: flags.verbose }
  );
  
  runner.fixtures = fixtures;
  
  const results = await runner.run(suites, caseFilter);
  
  console.log('\n═══════════════════════════════════════');
  console.log('SUMMARY');
  console.log('═══════════════════════════════════════');
  console.log(`Passed: ${results.passed}`);
  console.log(`Failed: ${results.failed}`);
  console.log(`Total:  ${results.passed + results.failed}`);
  
  if (results.suiteTimes && results.suiteTimes.length > 0) {
    console.log('\nSuite Times:');
    for (const st of results.suiteTimes) {
      console.log(`  ${st.name}: ${st.time}ms`);
    }
  }
  console.log('');
  
  process.exit(results.failed > 0 ? 1 : 0);
}

main().catch(e => {
  console.error('Fatal error:', e);
  process.exit(1);
});