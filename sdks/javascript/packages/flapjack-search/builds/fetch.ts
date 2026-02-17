// Flapjack Search — fetch build
// Re-exports @flapjack-search/client-search fetch build with a friendlier factory name.

import type { ClientOptions } from '@flapjack-search/client-common';
import { searchClient } from '@flapjack-search/client-search';
import type { SearchClient } from '@flapjack-search/client-search';

export type { SearchClient } from '@flapjack-search/client-search';
export { apiClientVersion } from '@flapjack-search/client-search';
export * from './models';

export type FlapjackSearch = SearchClient & {
  get _ua(): string;
};

export function flapjackSearch(
  appId: string,
  apiKey: string,
  options?: ClientOptions | undefined,
): FlapjackSearch {
  const client = searchClient(appId, apiKey, options);

  return {
    ...client,
    get _ua(): string {
      return client.transporter.flapjackAgent.value;
    },
  };
}
