import * as dotenv from 'dotenv';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Playwright global setup — loads environment variables from .secret/.env.secret
 * so integration tests can access ALGOLIA_APP_ID and ALGOLIA_ADMIN_KEY.
 */
export default function globalSetup() {
  dotenv.config({
    path: join(__dirname, '..', '..', '.secret', '.env.secret'),
  });
}
