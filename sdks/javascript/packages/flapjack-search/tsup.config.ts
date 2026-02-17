import type { Options } from 'tsup';
import { defineConfig } from 'tsup';

import { getBaseNodeOptions, getBaseBrowserOptions, getDependencies } from '../../base.tsup.config';

import pkg from './package.json' with { type: 'json' };

const nodeOptions: Options = {
  ...getBaseNodeOptions(pkg, __dirname),
  dts: { entry: { node: 'builds/node.ts' } },
  entry: ['builds/node.ts'],
};

const nodeConfigs: Options[] = [
  {
    ...nodeOptions,
    format: 'cjs',
    name: `node ${pkg.name} cjs`,
  },
  {
    ...nodeOptions,
    format: 'esm',
    name: `node ${pkg.name} esm`,
  },
  {
    ...nodeOptions,
    format: 'esm',
    name: `fetch ${pkg.name} esm`,
    dts: { entry: { fetch: 'builds/fetch.ts' } },
    entry: ['builds/fetch.ts'],
    external: getDependencies(pkg, 'fetch'),
  },
  {
    ...nodeOptions,
    format: 'esm',
    name: `worker ${pkg.name} esm`,
    dts: { entry: { worker: 'builds/worker.ts' } },
    entry: ['builds/worker.ts'],
    external: getDependencies(pkg, 'worker'),
  },
];

const browserOptions: Options = {
  ...getBaseBrowserOptions(pkg, __dirname),
  globalName: 'flapjackSearch',
};

const browserConfigs: Options[] = [
  {
    ...browserOptions,
    minify: false,
    name: `browser ${pkg.name} esm`,
    dts: { entry: { browser: 'builds/browser.ts' } },
    entry: ['builds/browser.ts'],
  },
  {
    ...browserOptions,
    dts: false,
    minify: true,
    name: `browser ${pkg.name} min esm`,
    entry: { 'browser.min': 'builds/browser.ts' },
    external: [],
    noExternal: getDependencies(pkg, 'xhr'),
  },
];

export default defineConfig([...nodeConfigs, ...browserConfigs]);
