=== Flapjack Search ===
Contributors: flapjackhq
Tags: search, instant search, relevance, autocomplete, faceted search
Requires at least: 6.4
Tested up to: 6.9
Requires PHP: 7.4
Stable tag: 0.1.0-beta
License: GPLv2 or later
License URI: https://www.gnu.org/licenses/gpl-2.0.html

Fast, typo-tolerant search for WordPress powered by Flapjack. (Beta)

== Description ==

**This plugin is in beta (v0.1.0).** We recommend testing on a staging site before using in production. Feedback and bug reports are welcome at [our GitHub repository](https://github.com/flapjackhq/flapjack-search-wordpress).

Flapjack Search replaces the default WordPress search with a fast, typo-tolerant search engine. Get relevant results for your visitors with as-you-type instant search.

**Features:**

* **Backend search replacement** — Transparently replaces WordPress native search via `posts_pre_query` hook
* **Instant search** — Optional InstantSearch.js integration for as-you-type results
* **Autocomplete** — Drop-in autocomplete on existing search forms
* **Gutenberg block** — Native search block with live preview in the editor
* **Real-time sync** — Automatically syncs posts on save, delete, and status changes
* **Bulk reindex** — Reindex all content with one click or via WP-CLI
* **Background reindex** — Large-site friendly batch reindexing with Action Scheduler support and progress tracking
* **Atomic reindex** — Zero-downtime reindexing via temporary index and swap
* **WooCommerce faceted search** — Price range slider, category filters, stock/sale toggles, and star rating filters
* **WooCommerce ready** — Index products with prices, SKUs, categories, and attributes
* **REST API** — Full REST API for search, indexing, and status
* **WP-CLI** — Command-line tools for reindexing and management
* **Extensible** — Filter hooks for customizing records, search params, facet widgets, and settings

== Installation ==

1. Upload the `flapjack-search` folder to `/wp-content/plugins/`
2. Activate the plugin through the 'Plugins' menu in WordPress
3. Go to Settings > Flapjack Search
4. Enter your Flapjack API credentials
5. Click "Test Connection" to verify
6. Click "Reindex All Content" to build your initial search index

For large sites with thousands of posts, use the "Background Reindex" option which processes content in batches. If Action Scheduler is available (included with WooCommerce), it will be used automatically for reliable background processing.

== Frequently Asked Questions ==

= Do I need a Flapjack account? =

Yes. You can sign up at https://flapjack.io for a free tier, or self-host Flapjack.

= Can I use this with a self-hosted Flapjack instance? =

Yes! Enter your server URL in the "Custom Host" field in settings.

= Does this work with WooCommerce? =

Yes. Add "product" to the indexed post types in settings and reindex. You will automatically get faceted search with price range sliders, category filters, stock/sale toggles, and star ratings on the InstantSearch overlay.

= How does background reindexing work? =

Background reindexing processes your content in batches of 200 posts. If Action Scheduler (bundled with WooCommerce) is available, it uses that for reliable background processing. Otherwise, it falls back to WP-Cron. You can monitor progress and cancel at any time from the settings page.

= Will reindexing cause downtime? =

No. The plugin uses atomic reindexing — it builds a new index in the background and swaps it in when complete. Your visitors continue to search against the existing index during the process.

== Screenshots ==

1. Admin settings page with connection testing and reindex controls
2. InstantSearch overlay with faceted WooCommerce filters
3. Gutenberg search block in the editor

== Changelog ==

= 0.1.0 =
* Initial release
* Backend search replacement via posts_pre_query
* InstantSearch.js frontend integration
* Gutenberg search block with live editor preview
* Real-time post sync on save/delete/status change
* Bulk reindex via admin UI and WP-CLI
* Background reindex with Action Scheduler support and WP-Cron fallback
* Atomic reindex with zero-downtime index swap
* WooCommerce product indexing with prices, SKUs, categories, and attributes
* WooCommerce faceted search: price slider, category refinement, stock/sale toggles, star ratings
* REST API endpoints for search, index, and status
* Admin settings page with connection testing and progress tracking
* Clean uninstall removing all options, transients, and scheduled events
