<?php
/**
 * Tests for Frontend\Assets.
 *
 * @package Flapjack\WordPress\Tests\Unit\Frontend
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Unit\Frontend;

use PHPUnit\Framework\TestCase;
use Flapjack\WordPress\Frontend\Assets;

class AssetsTest extends TestCase {

    private Assets $assets;

    protected function setUp(): void {
        wp_stubs_reset();
        $this->assets = new Assets();
    }

    // ─── Hook registration ───────────────────────────────────

    public function test_register_adds_enqueue_scripts_action(): void {
        global $wp_actions;
        $this->assets->register();

        $hook_names = array_keys( $wp_actions );
        $this->assertContains( 'wp_enqueue_scripts', $hook_names );
    }

    public function test_register_adds_wp_footer_action(): void {
        global $wp_actions;
        $this->assets->register();

        $hook_names = array_keys( $wp_actions );
        $this->assertContains( 'wp_footer', $hook_names );
    }

    // ─── Enqueue guard conditions ────────────────────────────

    public function test_should_not_enqueue_when_instant_search_disabled(): void {
        global $wp_enqueued_scripts;
        update_option( 'flapjack_enable_instant', false );
        update_option( 'flapjack_app_id', 'test-app-id' );

        $this->assets->enqueue_frontend_assets();
        $this->assertEmpty( $wp_enqueued_scripts );
    }

    public function test_should_not_enqueue_when_app_id_not_set(): void {
        global $wp_enqueued_scripts;
        update_option( 'flapjack_enable_instant', true );
        // flapjack_app_id not set.

        $this->assets->enqueue_frontend_assets();
        $this->assertEmpty( $wp_enqueued_scripts );
    }

    // ─── Successful enqueue ──────────────────────────────────

    public function test_enqueue_registers_all_required_scripts(): void {
        global $wp_enqueued_scripts;
        $this->enable_instant_search_with_keys();

        $this->assets->enqueue_frontend_assets();

        $this->assertArrayHasKey( 'flapjack-instantsearch', $wp_enqueued_scripts );
        $this->assertArrayHasKey( 'flapjack-search-frontend', $wp_enqueued_scripts );
    }

    public function test_enqueue_registers_all_required_styles(): void {
        global $wp_enqueued_styles;
        $this->enable_instant_search_with_keys();

        $this->assets->enqueue_frontend_assets();

        $this->assertArrayHasKey( 'flapjack-search-frontend', $wp_enqueued_styles );
        $this->assertArrayHasKey( 'flapjack-instantsearch-theme', $wp_enqueued_styles );
    }

    public function test_enqueue_frontend_js_depends_on_instantsearch(): void {
        global $wp_enqueued_scripts;
        $this->enable_instant_search_with_keys();

        $this->assets->enqueue_frontend_assets();

        $frontend_deps = $wp_enqueued_scripts['flapjack-search-frontend']['deps'];
        $this->assertContains( 'flapjack-instantsearch', $frontend_deps );
    }

    public function test_enqueue_uses_local_vendor_paths_not_cdn(): void {
        global $wp_enqueued_scripts, $wp_enqueued_styles;
        $this->enable_instant_search_with_keys();

        $this->assets->enqueue_frontend_assets();

        // InstantSearch JS must be local, not CDN.
        $is_url = $wp_enqueued_scripts['flapjack-instantsearch']['src'];
        $this->assertStringContainsString( 'assets/vendor/instantsearch.production.min.js', $is_url );
        $this->assertStringNotContainsString( 'cdn.jsdelivr.net', $is_url );

        // InstantSearch CSS must be local, not CDN.
        $css_url = $wp_enqueued_styles['flapjack-instantsearch-theme']['src'];
        $this->assertStringContainsString( 'assets/vendor/instantsearch-satellite.min.css', $css_url );
        $this->assertStringNotContainsString( 'cdn.jsdelivr.net', $css_url );
    }

    public function test_enqueue_does_not_register_unused_sdk_client(): void {
        global $wp_enqueued_scripts;
        $this->enable_instant_search_with_keys();

        $this->assets->enqueue_frontend_assets();

        // The Flapjack SDK is not used by frontend.js (it creates its own REST client).
        $this->assertArrayNotHasKey( 'flapjack-search-client', $wp_enqueued_scripts );
    }

    public function test_enqueue_localizes_config_to_frontend_script(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        update_option( 'flapjack_host', 'http://localhost:7700' );
        update_option( 'flapjack_index_name', 'my_index' );
        update_option( 'flapjack_posts_per_page', 25 );

        $this->assets->enqueue_frontend_assets();

        $this->assertArrayHasKey( 'flapjack-search-frontend', $wp_localized_scripts );
        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertSame( 'flapjackSearchConfig', $wp_localized_scripts['flapjack-search-frontend']['object_name'] );
        $this->assertSame( 'test-app-id', $config['appId'] );
        $this->assertSame( 'test-search-key', $config['apiKey'] );
        $this->assertSame( 'my_index', $config['indexName'] );
        $this->assertSame( 25, $config['perPage'] );
        $this->assertNotEmpty( $config['restUrl'] );
        $this->assertNotEmpty( $config['nonce'] );
    }

    public function test_enqueue_skips_when_no_search_key_set(): void {
        global $wp_enqueued_scripts;
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'admin-key-should-never-leak' );
        update_option( 'flapjack_search_api_key', '' );

        $this->assets->enqueue_frontend_assets();

        // Must NOT enqueue anything — admin key must never be exposed to frontend.
        $this->assertEmpty( $wp_enqueued_scripts );
    }

    public function test_enqueue_with_default_index_name(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        // Don't set index_name — should default to wp_posts.

        $this->assets->enqueue_frontend_assets();

        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertSame( 'wp_posts', $config['indexName'] );
    }

    public function test_enqueue_with_default_posts_per_page(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        // Don't set posts_per_page — should default to 20.

        $this->assets->enqueue_frontend_assets();

        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertSame( 20, $config['perPage'] );
    }

    // ─── Instant search container ────────────────────────────

    public function test_render_instant_search_container_outputs_html_when_enabled(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack-instant-search-overlay', $output );
        $this->assertStringContainsString( 'flapjack-instant-search-modal', $output );
        $this->assertStringContainsString( 'flapjack-searchbox', $output );
        $this->assertStringContainsString( 'flapjack-hits', $output );
        $this->assertStringContainsString( 'flapjack-pagination', $output );
        $this->assertStringContainsString( 'flapjack-powered-by', $output );
    }

    public function test_render_instant_search_container_outputs_nothing_when_disabled(): void {
        update_option( 'flapjack_enable_instant', false );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertEmpty( $output );
    }

    public function test_render_instant_search_container_outputs_nothing_when_no_app_id(): void {
        update_option( 'flapjack_enable_instant', true );
        // No app_id set.

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertEmpty( $output );
    }

    public function test_should_enqueue_respects_filter(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );

        add_filter( 'flapjack_should_enqueue_instant_search', function () {
            return false;
        } );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertEmpty( $output );
    }

    public function test_filter_also_blocks_script_enqueue(): void {
        global $wp_enqueued_scripts;
        $this->enable_instant_search_with_keys();

        add_filter( 'flapjack_should_enqueue_instant_search', function () {
            return false;
        } );

        $this->assets->enqueue_frontend_assets();
        $this->assertEmpty( $wp_enqueued_scripts );
    }

    // ─── WooCommerce facet integration ──────────────────────

    public function test_enqueue_includes_woocommerce_config_when_facets_enabled(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        update_option( 'flapjack_post_types', [ 'post', 'product' ] );

        $this->assets->enqueue_frontend_assets();

        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertArrayHasKey( 'woocommerce', $config );
        $this->assertTrue( $config['woocommerce']['enabled'] );
    }

    public function test_enqueue_omits_woocommerce_config_when_product_not_indexed(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $this->assets->enqueue_frontend_assets();

        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertArrayNotHasKey( 'woocommerce', $config );
    }

    public function test_enqueue_woocommerce_config_includes_currency_symbol(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        update_option( 'flapjack_post_types', [ 'product' ] );
        wc_set_currency_symbol( '£' );

        $this->assets->enqueue_frontend_assets();

        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertSame( '£', $config['woocommerce']['currencySymbol'] );
    }

    public function test_enqueue_woocommerce_config_includes_widgets(): void {
        global $wp_localized_scripts;
        $this->enable_instant_search_with_keys();
        update_option( 'flapjack_post_types', [ 'product' ] );

        $this->assets->enqueue_frontend_assets();

        $config = $wp_localized_scripts['flapjack-search-frontend']['data'];
        $this->assertArrayHasKey( 'widgets', $config['woocommerce'] );
        $this->assertCount( 5, $config['woocommerce']['widgets'] );
    }

    public function test_render_container_includes_facet_sidebar_when_woocommerce_enabled(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_post_types', [ 'product' ] );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack-facets', $output );
        $this->assertStringContainsString( 'flapjack-facet-categories', $output );
        $this->assertStringContainsString( 'flapjack-facet-price', $output );
        $this->assertStringContainsString( 'flapjack-facet-stock', $output );
        $this->assertStringContainsString( 'flapjack-facet-sale', $output );
        $this->assertStringContainsString( 'flapjack-facet-rating', $output );
        $this->assertStringContainsString( 'flapjack-clear-refinements', $output );
    }

    public function test_render_container_omits_facet_sidebar_when_woocommerce_disabled(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_post_types', [ 'post', 'page' ] );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertStringNotContainsString( 'flapjack-facets', $output );
        $this->assertStringNotContainsString( 'flapjack-facet-categories', $output );
    }

    public function test_render_container_includes_stats_div(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack-stats', $output );
    }

    public function test_render_container_includes_layout_wrapper(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );

        ob_start();
        $this->assets->render_instant_search_container();
        $output = ob_get_clean();

        $this->assertStringContainsString( 'flapjack-instant-search-layout', $output );
        $this->assertStringContainsString( 'flapjack-results-main', $output );
    }

    // ─── Helpers ─────────────────────────────────────────────

    private function enable_instant_search_with_keys(): void {
        update_option( 'flapjack_enable_instant', true );
        update_option( 'flapjack_app_id', 'test-app-id' );
        update_option( 'flapjack_api_key', 'test-admin-key' );
        update_option( 'flapjack_search_api_key', 'test-search-key' );
    }
}
