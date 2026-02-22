/**
 * API helper functions for E2E tests.
 *
 * These wrap raw request.* calls so spec files don't need to use
 * request.get/post/delete directly (which is banned by ESLint).
 * Fixture files are exempt from the ESLint spec-file rules.
 */
import type { APIRequestContext } from '@playwright/test';
import { API_BASE, API_HEADERS } from './local-instance';

/** Delete an index. Ignores errors if index doesn't exist. */
export async function deleteIndex(
  request: APIRequestContext,
  indexName: string,
): Promise<void> {
  await request.delete(`${API_BASE}/1/indexes/${indexName}`, {
    headers: API_HEADERS,
  }).catch(() => {});
}

/** Add documents to an index via the batch API. */
export async function addDocuments(
  request: APIRequestContext,
  indexName: string,
  documents: Array<Record<string, unknown>>,
): Promise<void> {
  await request.post(`${API_BASE}/1/indexes/${indexName}/batch`, {
    headers: API_HEADERS,
    data: {
      requests: documents.map((doc) => ({ action: 'addObject', body: doc })),
    },
  });
}

/** Search an index. Returns the parsed JSON response body. */
export async function searchIndex(
  request: APIRequestContext,
  indexName: string,
  query: string,
): Promise<{ nbHits?: number; hits?: unknown[] }> {
  const res = await request.post(
    `${API_BASE}/1/indexes/${indexName}/query`,
    { headers: API_HEADERS, data: { query } },
  );
  return res.json();
}

/** Get all rules for an index. */
export async function getRules(
  request: APIRequestContext,
  indexName: string,
): Promise<{ ok: boolean; items: any[] }> {
  const res = await request.get(
    `${API_BASE}/1/indexes/${indexName}/rules`,
    { headers: API_HEADERS },
  );
  if (!res.ok()) return { ok: false, items: [] };
  const body = await res.json();
  return { ok: true, items: body.hits || body.items || body };
}

/** Delete a specific rule by objectID. */
export async function deleteRule(
  request: APIRequestContext,
  indexName: string,
  ruleId: string,
): Promise<void> {
  await request.delete(
    `${API_BASE}/1/indexes/${indexName}/rules/${ruleId}`,
    { headers: API_HEADERS },
  );
}

/** Get index settings. */
export async function getSettings(
  request: APIRequestContext,
  indexName: string,
): Promise<Record<string, unknown>> {
  const res = await request.get(
    `${API_BASE}/1/indexes/${indexName}/settings`,
    { headers: API_HEADERS },
  );
  return res.json();
}

/** Update index settings via PUT. */
export async function updateSettings(
  request: APIRequestContext,
  indexName: string,
  settings: Record<string, unknown>,
): Promise<void> {
  await request.put(
    `${API_BASE}/1/indexes/${indexName}/settings`,
    { headers: API_HEADERS, data: settings },
  );
}

/** Create or upsert a rule (PUT). Throws on failure. */
export async function createRule(
  request: APIRequestContext,
  indexName: string,
  rule: { objectID: string } & Record<string, unknown>,
): Promise<void> {
  const res = await request.put(
    `${API_BASE}/1/indexes/${indexName}/rules/${rule.objectID}`,
    { headers: API_HEADERS, data: rule },
  );
  if (!res.ok()) {
    throw new Error(`createRule failed (${res.status()}): ${await res.text()}`);
  }
}

/** Create or upsert a synonym (PUT). Throws on failure. */
export async function createSynonym(
  request: APIRequestContext,
  indexName: string,
  synonym: { objectID: string } & Record<string, unknown>,
): Promise<void> {
  const res = await request.put(
    `${API_BASE}/1/indexes/${indexName}/synonyms/${synonym.objectID}`,
    { headers: API_HEADERS, data: synonym },
  );
  if (!res.ok()) {
    throw new Error(`createSynonym failed (${res.status()}): ${await res.text()}`);
  }
}

/** Delete a synonym by objectID. */
export async function deleteSynonym(
  request: APIRequestContext,
  indexName: string,
  synonymId: string,
): Promise<void> {
  await request.delete(
    `${API_BASE}/1/indexes/${indexName}/synonyms/${synonymId}`,
    { headers: API_HEADERS },
  ).catch(() => {});
}

// ---------------------------------------------------------------------------
// Vector search helpers
// ---------------------------------------------------------------------------

/** Configure a single embedder via PUT settings (whole-map replacement). */
export async function configureEmbedder(
  request: APIRequestContext,
  indexName: string,
  embedderName: string,
  config: Record<string, unknown>,
): Promise<void> {
  const res = await request.put(
    `${API_BASE}/1/indexes/${indexName}/settings`,
    { headers: API_HEADERS, data: { embedders: { [embedderName]: config } } },
  );
  if (!res.ok()) {
    throw new Error(`configureEmbedder failed (${res.status()}): ${await res.text()}`);
  }
}

/** Add documents that include _vectors field via the batch API. */
export async function addDocumentsWithVectors(
  request: APIRequestContext,
  indexName: string,
  documents: Array<Record<string, unknown>>,
): Promise<void> {
  const res = await request.post(`${API_BASE}/1/indexes/${indexName}/batch`, {
    headers: API_HEADERS,
    data: {
      requests: documents.map((doc) => ({ action: 'addObject', body: doc })),
    },
  });
  if (!res.ok()) {
    throw new Error(`addDocumentsWithVectors failed (${res.status()}): ${await res.text()}`);
  }
}

/** Clear all embedders by setting embedders to empty map. */
export async function clearEmbedders(
  request: APIRequestContext,
  indexName: string,
): Promise<void> {
  await request.put(
    `${API_BASE}/1/indexes/${indexName}/settings`,
    { headers: API_HEADERS, data: { embedders: {} } },
  );
}

// ---------------------------------------------------------------------------
// Experiments helpers
// ---------------------------------------------------------------------------

export interface CreateExperimentPayload {
  name: string;
  indexName: string;
  trafficSplit: number;
  control: Record<string, unknown>;
  variant: Record<string, unknown>;
  primaryMetric: string;
  minimumDays?: number;
}

export interface ExperimentRecord {
  id: string;
  name: string;
  status: string;
  [key: string]: unknown;
}

/** List all experiments via GET /2/abtests. Returns the array of experiments. */
export async function listExperiments(
  request: APIRequestContext,
): Promise<ExperimentRecord[]> {
  const res = await request.get(`${API_BASE}/2/abtests`, { headers: API_HEADERS });
  if (!res.ok()) {
    throw new Error(`listExperiments failed (${res.status()}): ${await res.text()}`);
  }
  const body = await res.json();
  return body.abtests || [];
}

/** Find an experiment by name. Throws if not found. */
export async function getExperimentByName(
  request: APIRequestContext,
  name: string,
): Promise<ExperimentRecord> {
  const experiments = await listExperiments(request);
  const matches = experiments.filter((e) => e.name === name);
  if (matches.length === 0) {
    throw new Error(`No experiment found with name "${name}"`);
  }
  if (matches.length > 1) {
    const ids = matches.map((e) => e.id).join(', ');
    throw new Error(`Multiple experiments found with name "${name}": ${ids}`);
  }
  return matches[0];
}

/** Create an experiment via POST /2/abtests. Throws on failure. */
export async function createExperiment(
  request: APIRequestContext,
  payload: CreateExperimentPayload,
): Promise<ExperimentRecord> {
  const res = await request.post(`${API_BASE}/2/abtests`, {
    headers: API_HEADERS,
    data: payload,
  });
  if (!res.ok()) {
    throw new Error(`createExperiment failed (${res.status()}): ${await res.text()}`);
  }
  return res.json();
}

/** Start an experiment via POST /2/abtests/:id/start. Throws on failure. */
export async function startExperiment(
  request: APIRequestContext,
  experimentId: string,
): Promise<ExperimentRecord> {
  const res = await request.post(`${API_BASE}/2/abtests/${experimentId}/start`, {
    headers: API_HEADERS,
  });
  if (!res.ok()) {
    throw new Error(`startExperiment failed (${res.status()}): ${await res.text()}`);
  }
  return res.json();
}

/** Stop an experiment via POST /2/abtests/:id/stop. Throws on failure. */
export async function stopExperiment(
  request: APIRequestContext,
  experimentId: string,
): Promise<ExperimentRecord> {
  const res = await request.post(`${API_BASE}/2/abtests/${experimentId}/stop`, {
    headers: API_HEADERS,
  });
  if (!res.ok()) {
    throw new Error(`stopExperiment failed (${res.status()}): ${await res.text()}`);
  }
  return res.json();
}

export interface ConcludeExperimentPayload {
  winner: string | null;
  reason: string;
  controlMetric: number;
  variantMetric: number;
  confidence: number;
  significant: boolean;
  promoted: boolean;
}

/** Conclude an experiment via POST /2/abtests/:id/conclude. Throws on failure. */
export async function concludeExperiment(
  request: APIRequestContext,
  experimentId: string,
  payload: ConcludeExperimentPayload,
): Promise<ExperimentRecord> {
  const res = await request.post(`${API_BASE}/2/abtests/${experimentId}/conclude`, {
    headers: API_HEADERS,
    data: payload,
  });
  if (!res.ok()) {
    throw new Error(`concludeExperiment failed (${res.status()}): ${await res.text()}`);
  }
  return res.json();
}

/** Delete an experiment, stopping first if it is running. */
export async function deleteExperiment(
  request: APIRequestContext,
  experimentId: string,
): Promise<void> {
  const url = `${API_BASE}/2/abtests/${experimentId}`;
  const firstDelete = await request.delete(url, {
    headers: API_HEADERS,
  });

  if (firstDelete.ok() || firstDelete.status() === 404) {
    return;
  }

  if (firstDelete.status() === 409) {
    const stopRes = await request.post(`${url}/stop`, {
      headers: API_HEADERS,
    });
    if (!stopRes.ok() && stopRes.status() !== 409 && stopRes.status() !== 404) {
      throw new Error(`stopExperiment before delete failed (${stopRes.status()}): ${await stopRes.text()}`);
    }

    const retryDelete = await request.delete(url, {
      headers: API_HEADERS,
    });
    if (retryDelete.ok() || retryDelete.status() === 404) {
      return;
    }
    throw new Error(`deleteExperiment retry failed (${retryDelete.status()}): ${await retryDelete.text()}`);
  }

  throw new Error(`deleteExperiment failed (${firstDelete.status()}): ${await firstDelete.text()}`);
}

/** Delete all experiments whose name starts with the provided prefix. */
export async function deleteExperimentsByPrefix(
  request: APIRequestContext,
  prefix: string,
): Promise<void> {
  const experiments = await listExperiments(request);
  for (const exp of experiments) {
    if (typeof exp.name === 'string' && exp.name.startsWith(prefix)) {
      await deleteExperiment(request, exp.id);
    }
  }
}

/** Delete all experiments with an exact name match. */
export async function deleteExperimentsByName(
  request: APIRequestContext,
  name: string,
): Promise<void> {
  const experiments = await listExperiments(request);
  for (const exp of experiments) {
    if (exp.name === name) {
      await deleteExperiment(request, exp.id);
    }
  }
}
