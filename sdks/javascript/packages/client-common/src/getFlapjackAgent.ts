import { createFlapjackAgent } from './createFlapjackAgent';
import type { FlapjackAgent, FlapjackAgentOptions } from './types';

export type GetFlapjackAgent = {
  flapjackAgents: FlapjackAgentOptions[];
  client: string;
  version: string;
};

export function getFlapjackAgent({ flapjackAgents, client, version }: GetFlapjackAgent): FlapjackAgent {
  const defaultFlapjackAgent = createFlapjackAgent(version).add({
    segment: client,
    version,
  });

  flapjackAgents.forEach((flapjackAgent) => defaultFlapjackAgent.add(flapjackAgent));

  return defaultFlapjackAgent;
}
