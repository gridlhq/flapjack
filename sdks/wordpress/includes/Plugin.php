<?php
/**
 * Main Plugin class — singleton entry point.
 *
 * @package Flapjack\WordPress
 */

declare(strict_types=1);

namespace Flapjack\WordPress;

use Flapjack\WordPress\Admin\SettingsPage;
use Flapjack\WordPress\Indexing\BackgroundIndexer;
use Flapjack\WordPress\Indexing\IndexManager;
use Flapjack\WordPress\Indexing\PostSyncHooks;
use Flapjack\WordPress\Search\QueryInterceptor;
use Flapjack\WordPress\Frontend\Assets;
use Flapjack\WordPress\REST\SearchEndpoint;
use Flapjack\WordPress\REST\IndexEndpoint;
use Flapjack\WordPress\REST\StatusEndpoint;
use Flapjack\WordPress\Block\SearchBlock;
use Flapjack\WordPress\CLI\Commands;
use Flapjack\WordPress\WooCommerce\ProductIntegration;

final class Plugin {

    private static ?self $instance = null;

    private ?ClientFactory $client_factory = null;

    private function __construct() {}

    public static function get_instance(): self {
        if ( null === self::$instance ) {
            self::$instance = new self();
        }
        return self::$instance;
    }

    /**
     * Initialize all plugin components.
     */
    public function init(): void {
        $this->client_factory = new ClientFactory();

        // Admin.
        if ( is_admin() ) {
            $settings = new SettingsPage();
            $settings->register();
        }

        // Indexing hooks — always active so saves/deletes sync.
        $index_manager = new IndexManager( $this->client_factory );
        $post_sync     = new PostSyncHooks( $index_manager, $this->client_factory );
        $post_sync->register();

        // Background reindex handler (Action Scheduler / WP-Cron).
        $background_indexer = new BackgroundIndexer( $this->client_factory );
        $background_indexer->register();

        // Search interception.
        $query_interceptor = new QueryInterceptor( $this->client_factory );
        $query_interceptor->register();

        // Frontend assets.
        $assets = new Assets();
        $assets->register();

        // Gutenberg search block.
        $search_block = new SearchBlock();
        $search_block->register();

        // WooCommerce integration (conditional).
        if ( ProductIntegration::is_woocommerce_active() ) {
            $woocommerce = new ProductIntegration( $index_manager );
            $woocommerce->register();
        }

        // WP-CLI commands.
        Commands::register( $this->client_factory, $index_manager );

        // REST API.
        add_action( 'rest_api_init', function () use ( $index_manager ) {
            ( new SearchEndpoint( $this->client_factory ) )->register();
            ( new IndexEndpoint( $index_manager ) )->register();
            ( new StatusEndpoint( $this->client_factory, $index_manager ) )->register();
        } );

        // Load text domain.
        add_action( 'init', [ $this, 'load_textdomain' ] );
    }

    /**
     * Load plugin text domain for translations.
     */
    public function load_textdomain(): void {
        load_plugin_textdomain(
            'flapjack-search',
            false,
            dirname( FLAPJACK_SEARCH_BASENAME ) . '/languages'
        );
    }

    /**
     * Get the client factory.
     */
    public function get_client_factory(): ClientFactory {
        return $this->client_factory;
    }

    /**
     * Activation hook.
     */
    public static function activate(): void {
        // Set default options.
        $defaults = [
            'flapjack_app_id'           => '',
            'flapjack_api_key'          => '',
            'flapjack_search_api_key'   => '',
            'flapjack_host'             => '',
            'flapjack_index_name'       => 'wp_posts',
            'flapjack_post_types'       => [ 'post', 'page' ],
            'flapjack_enable_search'    => true,
            'flapjack_enable_instant'   => false,
            'flapjack_posts_per_page'   => 20,
            'flapjack_searchable_attrs' => [ 'post_title', 'post_content', 'post_excerpt' ],
        ];

        foreach ( $defaults as $key => $value ) {
            if ( false === get_option( $key ) ) {
                add_option( $key, $value );
            }
        }

        // Set a transient for admin notice on activation.
        set_transient( 'flapjack_search_activated', true, 30 );

        // Flush rewrite rules for any custom endpoints.
        flush_rewrite_rules();
    }

    /**
     * Deactivation hook.
     */
    public static function deactivate(): void {
        // Clean up scheduled events.
        wp_clear_scheduled_hook( 'flapjack_search_reindex_cron' );
        wp_clear_scheduled_hook( BackgroundIndexer::BATCH_HOOK );

        // Cancel any in-progress background reindex.
        delete_transient( BackgroundIndexer::PROGRESS_TRANSIENT );

        flush_rewrite_rules();
    }
}