// Re-export all types from client-search
export * from '@flapjack-search/client-search';

// Re-export common types that users frequently need
export type { ClientOptions, RequestOptions } from '@flapjack-search/client-common';

export type InitClientOptions = Partial<{
  appId?: string;
  apiKey?: string;
  options?: import('@flapjack-search/client-common').ClientOptions;
}>;
