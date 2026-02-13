import type { FlapjackAgent, FlapjackAgentOptions } from './types';

export function createFlapjackAgent(version: string): FlapjackAgent {
  const flapjackAgent = {
    value: `Flapjack for JavaScript (${version})`,
    add(options: FlapjackAgentOptions): FlapjackAgent {
      const addedFlapjackAgent = `; ${options.segment}${options.version !== undefined ? ` (${options.version})` : ''}`;

      if (flapjackAgent.value.indexOf(addedFlapjackAgent) === -1) {
        flapjackAgent.value = `${flapjackAgent.value}${addedFlapjackAgent}`;
      }

      return flapjackAgent;
    },
  };

  return flapjackAgent;
}
