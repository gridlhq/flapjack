import { describe, expect, it } from 'vitest';
import {
  createMemoryCache,
  createNullCache,
  createNullLogger,
} from '@flapjack-search/client-common';
import { nodeEchoRequester } from '../../../requester-testing/src/nodeEchoRequester';
import { createSearchClient } from '../searchClient';

function createTestClient() {
  return createSearchClient({
    appId: 'test-app-id',
    apiKey: 'test-api-key',
    timeouts: {
      connect: 2000,
      read: 5000,
      write: 30000,
    },
    logger: createNullLogger(),
    requester: nodeEchoRequester(),
    flapjackAgents: [{ segment: 'Node.js', version: process.versions.node }],
    responsesCache: createNullCache(),
    requestsCache: createNullCache(),
    hostsCache: createMemoryCache(),
  });
}

describe('A/B testing SDK methods', () => {
  it('addABTest(body) sends POST /2/abtests with body', async () => {
    const client = createTestClient();
    const body = { name: 'exp-1', index: 'products' };

    const result = await client.addABTest(body);

    expect(result.path).toBe('/2/abtests');
    expect(result.method).toBe('POST');
    expect(result.data).toEqual(body);
  });

  it('getABTest(id) sends GET /2/abtests/{id}', async () => {
    const client = createTestClient();

    const result = await client.getABTest('123');

    expect(result.path).toBe('/2/abtests/123');
    expect(result.method).toBe('GET');
  });

  it('listABTests(params) sends GET /2/abtests with optional offset and limit', async () => {
    const client = createTestClient();

    const result = await client.listABTests({ offset: 10, limit: 20 });

    expect(result.path).toBe('/2/abtests');
    expect(result.method).toBe('GET');
    expect(result.searchParams).toEqual({ offset: '10', limit: '20' });
  });

  it('getABTestResults(id) sends GET /2/abtests/{id}/results', async () => {
    const client = createTestClient();

    const result = await client.getABTestResults('exp-42');

    expect(result.path).toBe('/2/abtests/exp-42/results');
    expect(result.method).toBe('GET');
  });

  it('stopABTest(id) sends POST /2/abtests/{id}/stop', async () => {
    const client = createTestClient();

    const result = await client.stopABTest('exp-42');

    expect(result.path).toBe('/2/abtests/exp-42/stop');
    expect(result.method).toBe('POST');
  });

  it('deleteABTest(id) sends DELETE /2/abtests/{id}', async () => {
    const client = createTestClient();

    const result = await client.deleteABTest('exp-42');

    expect(result.path).toBe('/2/abtests/exp-42');
    expect(result.method).toBe('DELETE');
  });
});

describe('A/B testing SDK parameter validation', () => {
  it('addABTest throws when body is missing', () => {
    const client = createTestClient();

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect(() => client.addABTest(undefined as any)).toThrow(
      'Parameter `addABTestRequest` is required when calling `addABTest`.'
    );
  });

  it('getABTest throws when id is empty', () => {
    const client = createTestClient();

    expect(() => client.getABTest('')).toThrow(
      'Parameter `abTestID` is required when calling `getABTest`.'
    );
  });

  it('getABTestResults throws when id is empty', () => {
    const client = createTestClient();

    expect(() => client.getABTestResults('')).toThrow(
      'Parameter `abTestID` is required when calling `getABTestResults`.'
    );
  });

  it('stopABTest throws when id is empty', () => {
    const client = createTestClient();

    expect(() => client.stopABTest('')).toThrow(
      'Parameter `abTestID` is required when calling `stopABTest`.'
    );
  });

  it('deleteABTest throws when id is empty', () => {
    const client = createTestClient();

    expect(() => client.deleteABTest('')).toThrow(
      'Parameter `abTestID` is required when calling `deleteABTest`.'
    );
  });
});
