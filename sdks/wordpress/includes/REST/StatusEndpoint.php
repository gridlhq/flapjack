<?php
/**
 * REST API endpoint for plugin status information.
 *
 * @package Flapjack\WordPress\REST
 */

declare(strict_types=1);

namespace Flapjack\WordPress\REST;

use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;

class StatusEndpoint {

    private const NAMESPACE = 'flapjack-search/v1';

    private ClientFactory $client_factory;
    private IndexManager $index_manager;

    public function __construct( ClientFactory $client_factory, IndexManager $index_manager ) {
        $this->client_factory = $client_factory;
        $this->index_manager  = $index_manager;
    }

    /**
     * Register the REST route.
     */
    public function register(): void {
        register_rest_route( self::NAMESPACE, '/status', [
            'methods'             => 'GET',
            'callback'            => [ $this, 'handle_status' ],
            'permission_callback' => [ $this, 'check_permission' ],
        ] );

        register_rest_route( self::NAMESPACE, '/test-connection', [
            'methods'             => 'POST',
            'callback'            => [ $this, 'handle_test_connection' ],
            'permission_callback' => [ $this, 'check_permission' ],
        ] );
    }

    /**
     * Handle status request.
     *
     * @param \WP_REST_Request $request
     * @return \WP_REST_Response
     */
    public function handle_status( \WP_REST_Request $request ): \WP_REST_Response {
        $index_stats = $this->index_manager->get_index_stats();
        $post_types  = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

        // Count indexable posts in WP.
        $wp_count = 0;
        foreach ( $post_types as $post_type ) {
            $counts = wp_count_posts( $post_type );
            $wp_count += (int) ( $counts->publish ?? 0 );
        }

        return new \WP_REST_Response( [
            'plugin_version'  => FLAPJACK_SEARCH_VERSION,
            'configured'      => $this->client_factory->is_configured(),
            'search_enabled'  => (bool) get_option( 'flapjack_enable_search', true ),
            'instant_enabled' => (bool) get_option( 'flapjack_enable_instant', false ),
            'index'           => $index_stats,
            'wp_post_count'   => $wp_count,
            'indexed_types'   => $post_types,
        ], 200 );
    }

    /**
     * Handle connection test.
     *
     * @param \WP_REST_Request $request
     * @return \WP_REST_Response
     */
    public function handle_test_connection( \WP_REST_Request $request ): \WP_REST_Response {
        $result = $this->client_factory->test_connection();
        $status = $result['success'] ? 200 : 503;
        return new \WP_REST_Response( $result, $status );
    }

    /**
     * Permission check — require manage_options.
     */
    public function check_permission(): bool {
        return current_user_can( 'manage_options' );
    }
}
