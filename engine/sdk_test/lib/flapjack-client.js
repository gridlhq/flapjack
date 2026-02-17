// Shared Flapjack client config for all SDK tests.
// Loads FLAPJACK_ADMIN_KEY from .env.secret so tests and dev-server use the same key.

import { algoliasearch } from 'algoliasearch';
import * as dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);


dotenv.config({ path: join(__dirname, '..', '..', '.secret', '.env.secret') });

const FLAPJACK_URL = process.env.FLAPJACK_URL || 'http://localhost:7700';
const FLAPJACK_ADMIN_KEY = process.env.FLAPJACK_ADMIN_KEY || 'fj_test_admin_key_for_local_dev';

function createFlapjackRequester(opts = {}) {
  const target = new URL(FLAPJACK_URL);
  return {
    async send(request) {
      const url = new URL(request.url);
      url.protocol = target.protocol;
      url.host = target.host;

      if (opts.verbose && request.data) {
        console.log('[FLAPJACK BODY]', request.data.slice(0, 300));
      }

      const response = await fetch(url.toString(), {
        method: request.method,
        headers: request.headers,
        body: request.data,
      });

      const text = await response.text();

      if (opts.debug) {
        console.log('REQUEST:', request.method, url.pathname, JSON.stringify(JSON.parse(request.data || '{}'), null, 2));
        console.log('RESPONSE:', response.status, text.substring(0, 200));
      }

      return {
        status: response.status,
        content: text,
        isTimedOut: false,
      };
    },
  };
}

export function createFlapjackClient(opts = {}) {
  return algoliasearch('flapjack', FLAPJACK_ADMIN_KEY, {
    requester: createFlapjackRequester(opts),
  });
}

export { FLAPJACK_URL, FLAPJACK_ADMIN_KEY };
