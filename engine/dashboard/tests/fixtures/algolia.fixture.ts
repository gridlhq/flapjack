import { algoliasearch } from 'algoliasearch';
import { PRODUCTS, SYNONYMS, RULES, SETTINGS } from './test-data';

export interface AlgoliaTestContext {
  appId: string;
  adminKey: string;
  indexName: string;
}

/**
 * Returns true if Algolia credentials are available in the environment.
 */
export function hasAlgoliaCredentials(): boolean {
  return !!(process.env.ALGOLIA_APP_ID && process.env.ALGOLIA_ADMIN_KEY);
}

/**
 * Seeds an Algolia index with known test data (products, settings, synonyms, rules).
 * Polls until all documents are searchable before returning.
 */
export async function seedAlgoliaIndex(): Promise<AlgoliaTestContext> {
  const appId = process.env.ALGOLIA_APP_ID!;
  const adminKey = process.env.ALGOLIA_ADMIN_KEY!;
  const indexName = `fj_e2e_migrate_${Date.now()}`;

  const client = algoliasearch(appId, adminKey);

  // Apply settings
  await client.setSettings({ indexName, indexSettings: SETTINGS });

  // Save synonyms
  for (const syn of SYNONYMS) {
    await client.saveSynonym({
      indexName,
      objectID: syn.objectID,
      synonymHit: syn as any,
    });
  }

  // Save rules
  for (const rule of RULES) {
    await client.saveRule({
      indexName,
      objectID: rule.objectID,
      rule: rule as any,
    });
  }

  // Save objects
  await client.saveObjects({ indexName, objects: PRODUCTS });

  // Poll until all documents are indexed and searchable
  await pollAlgoliaReady(client, indexName, PRODUCTS.length);

  return { appId, adminKey, indexName };
}

/**
 * Deletes the Algolia test index. Swallows errors for cleanup robustness.
 */
export async function deleteAlgoliaIndex(ctx: AlgoliaTestContext): Promise<void> {
  try {
    const client = algoliasearch(ctx.appId, ctx.adminKey);
    await client.deleteIndex({ indexName: ctx.indexName });
  } catch {
    // Best-effort cleanup
  }
}

/**
 * Deletes a Flapjack index via the REST API. Swallows errors for cleanup robustness.
 */
export async function deleteFlapjackIndex(
  indexName: string,
  baseUrl = 'http://localhost:7700',
): Promise<void> {
  try {
    await fetch(`${baseUrl}/1/indexes/${indexName}`, {
      method: 'DELETE',
      headers: {
        'x-algolia-api-key': 'abcdef0123456789',
        'x-algolia-application-id': 'flapjack',
      },
    });
  } catch {
    // Best-effort cleanup
  }
}

/**
 * Polls Algolia until the expected number of documents are searchable.
 */
async function pollAlgoliaReady(
  client: ReturnType<typeof algoliasearch>,
  indexName: string,
  expectedCount: number,
  maxWaitMs = 20_000,
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < maxWaitMs) {
    try {
      const result = await client.search({
        requests: [{ indexName, query: '' }],
      });
      const first = result.results[0];
      if ('nbHits' in first && (first as any).nbHits >= expectedCount) return;
    } catch {
      // Index may not exist yet — keep polling
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(
    `Algolia indexing timeout: expected ${expectedCount} docs in "${indexName}" after ${maxWaitMs}ms`,
  );
}
