<?php
/**
 * Frontend asset management — enqueues InstantSearch.js and custom CSS/JS.
 *
 * @package Flapjack\WordPress\Frontend
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Frontend;

use Flapjack\WordPress\WooCommerce\FacetConfig;

class Assets {

    /**
     * Register frontend hooks.
     */
    public function register(): void {
        add_action( 'wp_enqueue_scripts', [ $this, 'enqueue_frontend_assets' ] );
        add_action( 'wp_footer', [ $this, 'render_instant_search_container' ] );
    }

    /**
     * Enqueue frontend scripts and styles.
     */
    public function enqueue_frontend_assets(): void {
        if ( ! $this->should_enqueue() ) {
            return;
        }

        $app_id     = (string) get_option( 'flapjack_app_id', '' );
        $search_key = (string) get_option( 'flapjack_search_api_key', '' );
        $host       = (string) get_option( 'flapjack_host', '' );
        $index_name = (string) get_option( 'flapjack_index_name', 'wp_posts' );

        // Require a search-only API key for frontend use.
        // Never expose the admin API key in page source.
        if ( empty( $search_key ) ) {
            return;
        }

        // InstantSearch.js — bundled locally for WordPress.org compliance.
        wp_enqueue_script(
            'flapjack-instantsearch',
            FLAPJACK_SEARCH_URL . 'assets/vendor/instantsearch.production.min.js',
            [],
            '4.87.2',
            true
        );

        // Plugin custom JS.
        wp_enqueue_script(
            'flapjack-search-frontend',
            FLAPJACK_SEARCH_URL . 'assets/js/frontend.js',
            [ 'flapjack-instantsearch' ],
            FLAPJACK_SEARCH_VERSION,
            true
        );

        // Plugin custom CSS.
        wp_enqueue_style(
            'flapjack-search-frontend',
            FLAPJACK_SEARCH_URL . 'assets/css/frontend.css',
            [],
            FLAPJACK_SEARCH_VERSION
        );

        // InstantSearch default theme — bundled locally for WordPress.org compliance.
        wp_enqueue_style(
            'flapjack-instantsearch-theme',
            FLAPJACK_SEARCH_URL . 'assets/vendor/instantsearch-satellite.min.css',
            [],
            '8.10.0'
        );

        // Build configuration for JS.
        // Note: wp_localize_script handles encoding — do not use esc_js() here.
        $js_config = [
            'appId'     => $app_id,
            'apiKey'    => $search_key,
            'indexName' => $index_name,
            'host'      => $host,
            'perPage'   => (int) get_option( 'flapjack_posts_per_page', 20 ),
            'restUrl'   => esc_url_raw( rest_url( 'flapjack-search/v1/' ) ),
            'nonce'     => wp_create_nonce( 'wp_rest' ),
        ];

        // Add WooCommerce facet configuration if available.
        $facet_config = FacetConfig::get_config();
        if ( ! empty( $facet_config ) ) {
            $js_config['woocommerce'] = $facet_config;
        }

        wp_localize_script( 'flapjack-search-frontend', 'flapjackSearchConfig', $js_config );
    }

    /**
     * Render the InstantSearch container in the footer.
     */
    public function render_instant_search_container(): void {
        if ( ! $this->should_enqueue() ) {
            return;
        }
        ?>
        <div id="flapjack-instant-search-overlay" style="display:none;">
            <div id="flapjack-instant-search-modal">
                <div id="flapjack-searchbox"></div>
                <div id="flapjack-instant-search-layout">
                    <?php if ( FacetConfig::is_enabled() ) : ?>
                    <aside id="flapjack-facets" class="flapjack-facets-sidebar">
                        <div id="flapjack-facet-categories"></div>
                        <div id="flapjack-facet-price"></div>
                        <div id="flapjack-facet-stock"></div>
                        <div id="flapjack-facet-sale"></div>
                        <div id="flapjack-facet-rating"></div>
                        <div id="flapjack-clear-refinements"></div>
                    </aside>
                    <?php endif; ?>
                    <div id="flapjack-results-main">
                        <div id="flapjack-stats"></div>
                        <div id="flapjack-hits"></div>
                        <div id="flapjack-pagination"></div>
                    </div>
                </div>
                <div id="flapjack-powered-by"></div>
            </div>
        </div>
        <?php
    }

    /**
     * Determine if instant search assets should be enqueued.
     */
    private function should_enqueue(): bool {
        // Only if instant search is enabled.
        if ( ! get_option( 'flapjack_enable_instant', false ) ) {
            return false;
        }

        // Only if credentials are set.
        if ( empty( get_option( 'flapjack_app_id' ) ) ) {
            return false;
        }

        /**
         * Filter whether to enqueue InstantSearch assets.
         *
         * @param bool $should_enqueue
         */
        return (bool) apply_filters( 'flapjack_should_enqueue_instant_search', true );
    }
}
