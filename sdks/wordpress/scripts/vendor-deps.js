/**
 * Copies pre-built vendor dependencies to assets/vendor/ for local loading.
 *
 * WordPress.org prohibits loading JS/CSS from external CDNs.
 * This script copies the production builds from node_modules to local paths.
 */

import { copyFileSync, mkdirSync, readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const vendorDir = join(root, 'assets', 'vendor');

mkdirSync(vendorDir, { recursive: true });

const files = [
  {
    src: 'node_modules/instantsearch.js/dist/instantsearch.production.min.js',
    dest: 'instantsearch.production.min.js',
    label: 'InstantSearch.js (production UMD)',
  },
  {
    src: 'node_modules/instantsearch.css/themes/satellite-min.css',
    dest: 'instantsearch-satellite.min.css',
    label: 'InstantSearch CSS (satellite theme)',
  },
];

for (const file of files) {
  const srcPath = join(root, file.src);
  const destPath = join(vendorDir, file.dest);
  try {
    copyFileSync(srcPath, destPath);
    const size = readFileSync(destPath).length;
    console.log(`  ✓ ${file.label} → assets/vendor/${file.dest} (${(size / 1024).toFixed(1)}KB)`);
  } catch (err) {
    console.error(`  ✗ ${file.label}: ${err.message}`);
    process.exit(1);
  }
}

console.log('\nVendor dependencies copied successfully.');
