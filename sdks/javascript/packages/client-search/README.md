<p align="center">
  <a href="https://www.flapjack.com">
    <img alt="Flapjack for JavaScript" src="https://raw.githubusercontent.com/flapjackhq/flapjack-search-client-common/master/banners/javascript.png" >
  </a>

  <h4 align="center">The perfect starting point to integrate <a href="https://flapjack.com" target="_blank">Flapjack</a> within your JavaScript project</h4>

  <p align="center">
    <a href="https://npmjs.com/package/@flapjack-search/client-search"><img src="https://img.shields.io/npm/v/@flapjack-search/client-search.svg?style=flat-square" alt="NPM version"></img></a>
    <a href="http://npm-stat.com/charts.html?package=@flapjack-search/client-search"><img src="https://img.shields.io/npm/dm/@flapjack-search/client-search.svg?style=flat-square" alt="NPM downloads"></a>
    <a href="https://www.jsdelivr.com/package/npm/@flapjack-search/client-search"><img src="https://data.jsdelivr.com/v1/package/npm/@flapjack-search/client-search/badge" alt="jsDelivr Downloads"></img></a>
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

> [!TIP]
> This API client is already a dependency of [the flapjack-search client](https://www.npmjs.com/package/flapjack-search), you don't need to manually install `@flapjack-search/client-search` if you already have `flapjack-search` installed.

To get started, you first need to install @flapjack-search/client-search (or any other available API client package).
All of our clients comes with type definition, and are available for both browser and node environments.

### With a package manager


```bash
yarn add @flapjack-search/client-search@beta
# or
npm install @flapjack-search/client-search@beta
# or
pnpm add @flapjack-search/client-search@beta
```

### Without a package manager

Add the following JavaScript snippet to the <head> of your website:

```html
<script src="https://cdn.jsdelivr.net/npm/@flapjack-search/client-search/dist/builds/browser.umd.js"></script>
```

### Usage

You can now import the Flapjack API client in your project and play with it.

```js
import { searchClient } from '@flapjack-search/client-search';

const client = searchClient('YOUR_APP_ID', 'YOUR_API_KEY');
```

For full documentation, visit the **[Flapjack JavaScript API Client](https://www.flapjack.com/doc/libraries/sdk/methods/search/)**.

## ❓ Troubleshooting

Encountering an issue? Before reaching out to support, we recommend heading to our [FAQ](https://support.flapjack.com/hc/en-us/sections/15061037630609-API-Client-FAQs) where you will find answers for the most common issues and gotchas with the client. You can also open [a GitHub issue](https://github.com/flapjackhq/flapjack-search-automation/issues/new?assignees=&labels=&projects=&template=Bug_report.md)

## 📄 License

The Flapjack JavaScript API Client is an open-sourced software licensed under the [MIT license](LICENSE).