import type { Options } from 'tsup';
import { defineConfig } from 'tsup';

import { getBaseNodeOptions, getBaseBrowserOptions, getDependencies } from "../../base.tsup.config"

import pkg from "./package.json" with {type:'json'}

const nodeOptions: Options = {
  ...getBaseNodeOptions(pkg, __dirname),
  dts: { entry: { 'node': 'builds/node.ts' } },
  entry: ['builds/node.ts', 'src/*.ts'],
};

const nodeConfigs: Options[] = [
  {
    ...nodeOptions,
    format: 'cjs',
    name: `node ${pkg.name} cjs`
  },
  {
    ...nodeOptions,
    format: 'esm',
    name: `node ${pkg.name} esm`
  },
  {
    ...nodeOptions,
    format: 'esm',
    name: `fetch ${pkg.name} esm`,
    dts: { entry: { 'fetch': 'builds/fetch.ts' } },
    external: getDependencies(pkg, 'fetch'),
    entry: ['builds/fetch.ts', 'src/*.ts'],
  },
  {
    ...nodeOptions,
    format: 'esm',
    name: `worker ${pkg.name} esm`,
    dts: { entry: { 'worker': 'builds/worker.ts' } },
    external: getDependencies(pkg, 'worker'),
    entry: ['builds/worker.ts', 'src/*.ts'],
  },
];

const browserOptions: Options = {
  ...getBaseBrowserOptions(pkg, __dirname),
  globalName: 'searchClient',
};

const browserConfigs: Options[] = [
  {
    ...browserOptions,
    minify: false,
    name: `browser ${pkg.name} esm`,
    dts: { entry: { 'browser': 'builds/browser.ts' } },
    entry: ['builds/browser.ts', 'src/*.ts'],
  },
  {
    ...browserOptions,
    dts: false,
    minify: true,
    name: `browser ${pkg.name} min esm`,
    entry: {'builds/browser.min': 'builds/browser.ts'},
    external: ['@flapjack-search/requester-node-http', '@flapjack-search/requester-fetch', 'node:crypto', 'http', 'https'],
    noExternal: ['@flapjack-search/client-common', '@flapjack-search/requester-browser-xhr'],
  },
];

export default defineConfig([...nodeConfigs, ...browserConfigs]);