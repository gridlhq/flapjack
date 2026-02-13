import type { LogLevelType, Logger } from '@flapjack-search/client-common';
import { LogLevelEnum } from '@flapjack-search/client-common';

export function createConsoleLogger(logLevel: LogLevelType): Logger {
  return {
    debug(message: string, args?: any | undefined): Readonly<Promise<void>> {
      if (LogLevelEnum.Debug >= logLevel) {
        console.debug(message, args);
      }

      return Promise.resolve();
    },

    info(message: string, args?: any | undefined): Readonly<Promise<void>> {
      if (LogLevelEnum.Info >= logLevel) {
        console.info(message, args);
      }

      return Promise.resolve();
    },

    error(message: string, args?: any | undefined): Readonly<Promise<void>> {
      console.error(message, args);

      return Promise.resolve();
    },
  };
}
