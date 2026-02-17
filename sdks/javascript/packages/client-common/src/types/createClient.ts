import type { FlapjackAgentOptions, TransporterOptions } from './transporter';

export type AuthMode = 'WithinHeaders' | 'WithinQueryParameters';

type OverriddenTransporterOptions = 'baseHeaders' | 'baseQueryParameters' | 'hosts';

export type CreateClientOptions = Omit<TransporterOptions, OverriddenTransporterOptions | 'flapjackAgent'> &
  Partial<Pick<TransporterOptions, OverriddenTransporterOptions>> & {
    appId: string;
    apiKey: string;
    authMode?: AuthMode | undefined;
    flapjackAgents: FlapjackAgentOptions[];
  };

export type ClientOptions = Partial<Omit<CreateClientOptions, 'apiKey' | 'appId'>>;
