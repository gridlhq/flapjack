import { test, expect } from '@playwright/test';
import { API_BASE } from '../fixtures/local-instance';
import { deleteExperiment, getExperimentByName } from '../fixtures/api-helpers';

type FakeResponse = {
  ok: () => boolean;
  status: () => number;
  text: () => Promise<string>;
};

function response(status: number, body = ''): FakeResponse {
  return {
    ok: () => status >= 200 && status < 300,
    status: () => status,
    text: async () => body,
  };
}

test.describe('api-helpers deleteExperiment', () => {
  test('stops a running experiment and retries delete', async () => {
    const calls: string[] = [];
    const request = {
      delete: async (url: string) => {
        calls.push(`DELETE ${url}`);
        if (url.endsWith('/2/abtests/exp-running') && calls.length === 1) {
          return response(409, 'running');
        }
        return response(204);
      },
      post: async (url: string) => {
        calls.push(`POST ${url}`);
        return response(200);
      },
    } as any;

    await deleteExperiment(request, 'exp-running');

    expect(calls).toEqual([
      `DELETE ${API_BASE}/2/abtests/exp-running`,
      `POST ${API_BASE}/2/abtests/exp-running/stop`,
      `DELETE ${API_BASE}/2/abtests/exp-running`,
    ]);
  });

  test('ignores missing experiment', async () => {
    const request = {
      delete: async () => response(404),
      post: async () => response(200),
    } as any;

    await expect(deleteExperiment(request, 'does-not-exist')).resolves.toBeUndefined();
  });

  test('throws if stop fails before retrying delete', async () => {
    const request = {
      delete: async () => response(409, 'running'),
      post: async () => response(500, 'stop failed'),
    } as any;

    await expect(deleteExperiment(request, 'exp-running')).rejects.toThrow(
      /stopExperiment before delete failed/i,
    );
  });

  test('throws when multiple experiments share a name', async () => {
    const request = {
      get: async () => ({
        ok: () => true,
        status: () => 200,
        text: async () => '',
        json: async () => ({
          abtests: [
            { id: 'exp-1', name: 'dup-name', status: 'running' },
            { id: 'exp-2', name: 'dup-name', status: 'stopped' },
          ],
        }),
      }),
    } as any;

    await expect(getExperimentByName(request, 'dup-name')).rejects.toThrow(/multiple experiments found/i);
  });
});
