<p align="center">
  <a href="https://www.flapjack.com">
    <img alt="Flapjack for JavaScript" src="https://raw.githubusercontent.com/flapjackhq/flapjack-search-client-common/master/banners/javascript.png" >
  </a>

  <h4 align="center">The perfect starting point to integrate <a href="https://flapjack.com" target="_blank">Flapjack</a> within your JavaScript project</h4>

  <p align="center">
    <a href="https://npmjs.com/package/flapjack-search"><img src="https://img.shields.io/npm/v/flapjack-search.svg?style=flat-square" alt="NPM version"></img></a>
    <a href="http://npm-stat.com/charts.html?package=flapjack-search"><img src="https://img.shields.io/npm/dm/flapjack-search.svg?style=flat-square" alt="NPM downloads"></a>
    <a href="https://www.jsdelivr.com/package/npm/flapjack-search"><img src="https://data.jsdelivr.com/v1/package/npm/flapjack-search/badge" alt="jsDelivr Downloads"></img></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" alt="License"></a>
  </p>
</p>

<p align="center">
  <a href="https://www.flapjack.com/doc/libraries/sdk/install#javascript" target="_blank">Documentation</a>  •
  <a href="https://www.flapjack.com/doc/guides/building-search-ui/what-is-instantsearch/js/" target="_blank">InstantSearch</a>  •
  <a href="https://discourse.flapjack.com" target="_blank">Community Forum</a>  •
  <a href="http://stackoverflow.com/questions/tagged/flapjack" target="_blank">Stack Overflow</a>  •
  <a href="https://github.com/flapjackhq/flapjack-search-client-javascript/issues" target="_blank">Report a bug</a>  •
  <a href="https://flapjack.com/support" target="_blank">Support</a>
</p>

## ✨ Features

- Thin & **minimal low-level HTTP client** to interact with Flapjack's API
- Works both on the **browser** and **node.js**
- **UMD and ESM compatible**, you can use it with any module loader
- Built with TypeScript

## 💡 Getting Started

To get started, you first need to install flapjack-search (or any other available API client package).
All of our clients comes with type definition, and are available for both browser and node environments.

### With a package manager

```bash
yarn add flapjack-search@beta
# or
npm install flapjack-search@beta
# or
pnpm add flapjack-search@beta
```

### Without a package manager

Add the following JavaScript snippet to the <head> of your website:

```html
<script src="https://cdn.jsdelivr.net/npm/flapjack-search@beta/dist/flapjack-search.umd.js"></script>
```

### Usage

You can now import the Flapjack API client in your project and play with it.

```js
import { flapjackSearch } from 'flapjack-search';

const client = flapjackSearch('YOUR_APP_ID', 'YOUR_API_KEY');
```

### Self-hosted server

```js
const client = flapjackSearch('my-app', 'my-api-key', {
  hosts: [{ url: 'search.example.com', protocol: 'https', accept: 'readWrite' }],
});
```

### Drop-in InstantSearch compatibility

Flapjack Search works with the entire Algolia InstantSearch ecosystem — no widget changes needed:

```js
import { flapjackSearch } from 'flapjack-search';
import instantsearch from 'instantsearch.js';
import { searchBox, hits } from 'instantsearch.js/es/widgets';

const search = instantsearch({
  searchClient: flapjackSearch('APP_ID', 'API_KEY'),
  indexName: 'products',
});
search.addWidgets([searchBox({ container: '#search' }), hits({ container: '#hits' })]);
search.start();
```

Works with React InstantSearch, Vue InstantSearch, and Autocomplete.js too. See the [migration guide](https://github.com/flapjackhq/flapjack-search-javascript/blob/main/MIGRATION.md) for details.

### Migrating from Algolia?

Switching takes about 5 minutes:

1. `npm uninstall algoliasearch && npm install flapjack-search@beta`
2. Replace `import algoliasearch from 'algoliasearch'` with `import { flapjackSearch } from 'flapjack-search'`
3. Replace `algoliasearch(` with `flapjackSearch(`
4. Done — all methods, types, and widgets work identically.

## ❓ Troubleshooting

Encountering an issue? [Open a GitHub issue](https://github.com/flapjackhq/flapjack-search-javascript/issues/new) and we'll help.

## 📄 License

The Flapjack JavaScript API Client is an open-sourced software licensed under the [MIT license](LICENSE).
