<?php
/**
 * REST API endpoint for reindexing (admin only).
 *
 * @package Flapjack\WordPress\REST
 */

declare(strict_types=1);

namespace Flapjack\WordPress\REST;

use Flapjack\WordPress\Indexing\IndexManager;

class IndexEndpoint {

    private const NAMESPACE = 'flapjack-search/v1';

    private IndexManager $index_manager;

    public function __construct( IndexManager $index_manager ) {
        $this->index_manager = $index_manager;
    }

    /**
     * Register the REST route.
     */
    public function register(): void {
        register_rest_route( self::NAMESPACE, '/reindex', [
            'methods'             => 'POST',
            'callback'            => [ $this, 'handle_reindex' ],
            'permission_callback' => [ $this, 'check_permission' ],
        ] );

        register_rest_route( self::NAMESPACE, '/index/(?P<id>\d+)', [
            'methods'             => 'PUT',
            'callback'            => [ $this, 'handle_index_post' ],
            'permission_callback' => [ $this, 'check_permission' ],
            'args'                => [
                'id' => [
                    'type'              => 'integer',
                    'required'          => true,
                    'sanitize_callback' => 'absint',
                ],
            ],
        ] );

        register_rest_route( self::NAMESPACE, '/index/(?P<id>\d+)', [
            'methods'             => 'DELETE',
            'callback'            => [ $this, 'handle_delete_post' ],
            'permission_callback' => [ $this, 'check_permission' ],
            'args'                => [
                'id' => [
                    'type'              => 'integer',
                    'required'          => true,
                    'sanitize_callback' => 'absint',
                ],
            ],
        ] );
    }

    /**
     * Handle full reindex.
     *
     * @param \WP_REST_Request $request
     * @return \WP_REST_Response|\WP_Error
     */
    public function handle_reindex( \WP_REST_Request $request ) {
        try {
            $result = $this->index_manager->reindex_all();
            return new \WP_REST_Response( [
                'success' => true,
                'total'   => $result['total'],
                'batches' => $result['batches'],
            ], 200 );
        } catch ( \Throwable $e ) {
            return new \WP_Error(
                'flapjack_reindex_error',
                $e->getMessage(),
                [ 'status' => 500 ]
            );
        }
    }

    /**
     * Handle single post indexing.
     *
     * @param \WP_REST_Request $request
     * @return \WP_REST_Response|\WP_Error
     */
    public function handle_index_post( \WP_REST_Request $request ) {
        $post_id = (int) $request->get_param( 'id' );
        $post    = get_post( $post_id );

        if ( ! $post ) {
            return new \WP_Error(
                'flapjack_post_not_found',
                __( 'Post not found.', 'flapjack-search' ),
                [ 'status' => 404 ]
            );
        }

        try {
            $result = $this->index_manager->index_post( $post );
            return new \WP_REST_Response( [
                'success' => true,
                'result'  => $result,
            ], 200 );
        } catch ( \Throwable $e ) {
            return new \WP_Error(
                'flapjack_index_error',
                $e->getMessage(),
                [ 'status' => 500 ]
            );
        }
    }

    /**
     * Handle post deletion from index.
     *
     * @param \WP_REST_Request $request
     * @return \WP_REST_Response|\WP_Error
     */
    public function handle_delete_post( \WP_REST_Request $request ) {
        $post_id = (int) $request->get_param( 'id' );

        try {
            $result = $this->index_manager->delete_post( $post_id );
            return new \WP_REST_Response( [
                'success' => true,
                'result'  => $result,
            ], 200 );
        } catch ( \Throwable $e ) {
            return new \WP_Error(
                'flapjack_delete_error',
                $e->getMessage(),
                [ 'status' => 500 ]
            );
        }
    }

    /**
     * Permission check — require manage_options.
     */
    public function check_permission(): bool {
        return current_user_can( 'manage_options' );
    }
}
