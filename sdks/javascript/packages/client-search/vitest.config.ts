import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const currentDir = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  root: currentDir,
  resolve: {
    alias: {
      '@flapjack-search/client-common': path.resolve(
        currentDir,
        '../client-common/src/index.ts'
      ),
    },
  },
  test: {
    include: ['src/__tests__/**/*.test.ts'],
  },
});
