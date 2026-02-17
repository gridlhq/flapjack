<?php
/**
 * Intercepts WordPress search queries and routes them through Flapjack.
 *
 * Uses the posts_pre_query filter (WP 4.6+) to bypass WP_Query SQL entirely
 * when a search is detected, returning Flapjack results instead.
 *
 * @package Flapjack\WordPress\Search
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Search;

use Flapjack\WordPress\ClientFactory;

class QueryInterceptor {

    private ClientFactory $client_factory;

    public function __construct( ClientFactory $client_factory ) {
        $this->client_factory = $client_factory;
    }

    /**
     * Register the search interception hook.
     */
    public function register(): void {
        if ( ! $this->is_enabled() ) {
            return;
        }

        add_filter( 'posts_pre_query', [ $this, 'intercept_search' ], 10, 2 );
    }

    /**
     * Intercept a WP_Query search and route through Flapjack.
     *
     * @param array|null $posts Null to let WP_Query proceed normally, or array of posts to short-circuit.
     * @param \WP_Query  $query The WP_Query instance.
     * @return array|null
     */
    public function intercept_search( ?array $posts, \WP_Query $query ): ?array {
        // Only intercept main search queries.
        if ( ! $this->should_intercept( $query ) ) {
            return $posts;
        }

        $search_query = $query->get( 's' );
        if ( empty( $search_query ) ) {
            return $posts;
        }

        try {
            $result = $this->execute_search( $search_query, $query );
        } catch ( \Throwable $e ) {
            // On failure, fall back to native WordPress search.
            if ( defined( 'WP_DEBUG' ) && WP_DEBUG ) {
                error_log( sprintf( '[Flapjack Search] Query interception failed: %s', $e->getMessage() ) );
            }
            return $posts;
        }

        if ( empty( $result['hits'] ) ) {
            $query->found_posts   = 0;
            $query->max_num_pages = 0;
            return [];
        }

        // Map Flapjack hits back to WP_Post objects.
        $post_ids = array_map(
            fn( array $hit ) => (int) $hit['objectID'],
            $result['hits']
        );

        // Fetch the actual WP_Post objects in the correct order.
        $wp_posts = $this->get_posts_in_order( $post_ids );

        // Set pagination info on the query.
        $query->found_posts   = (int) ( $result['nbHits'] ?? count( $wp_posts ) );
        $hits_per_page        = (int) ( $result['hitsPerPage'] ?? $query->get( 'posts_per_page' ) );
        $query->max_num_pages = $hits_per_page > 0 ? (int) ceil( $query->found_posts / $hits_per_page ) : 1;

        // Store Flapjack metadata for templates.
        $query->set( 'flapjack_results', $result );

        return $wp_posts;
    }

    /**
     * Execute a search against the Flapjack API.
     *
     * @param string    $search_query
     * @param \WP_Query $query
     * @return array
     */
    public function execute_search( string $search_query, \WP_Query $query ): array {
        $client = $this->client_factory->get_client();
        $index  = $this->client_factory->get_index_name();

        $page     = max( 0, ( (int) $query->get( 'paged', 1 ) ) - 1 );
        $per_page = (int) $query->get( 'posts_per_page', get_option( 'flapjack_posts_per_page', 20 ) );

        if ( $per_page <= 0 ) {
            $per_page = 20;
        }

        $params = [
            'query'       => $search_query,
            'hitsPerPage' => $per_page,
            'page'        => $page,
        ];

        // Apply post type filter if specified.
        $post_type = $query->get( 'post_type' );
        $filters   = [];

        if ( ! empty( $post_type ) && 'any' !== $post_type ) {
            if ( is_array( $post_type ) ) {
                $type_filters = array_map(
                    fn( string $type ) => 'post_type:' . $type,
                    $post_type
                );
                $filters[]    = '(' . implode( ' OR ', $type_filters ) . ')';
            } else {
                $filters[] = 'post_type:' . $post_type;
            }
        }

        if ( ! empty( $filters ) ) {
            $params['filters'] = implode( ' AND ', $filters );
        }

        /**
         * Filter the Flapjack search parameters before the query is executed.
         *
         * @param array     $params The search parameters.
         * @param string    $search_query The original search string.
         * @param \WP_Query $query The WP_Query instance.
         */
        $params = (array) apply_filters( 'flapjack_search_params', $params, $search_query, $query );

        return $client->searchSingleIndex( $index, $params );
    }

    /**
     * Determine if we should intercept this query.
     */
    private function should_intercept( \WP_Query $query ): bool {
        // Don't intercept if the plugin isn't configured.
        if ( ! $this->client_factory->is_configured() ) {
            return false;
        }

        // Don't intercept if explicitly bypassed.
        if ( $query->get( 'flapjack_bypass' ) ) {
            return false;
        }

        // Only intercept search queries.
        if ( ! $query->is_search() ) {
            return false;
        }

        // Only intercept front-end (not admin) unless specifically enabled.
        if ( is_admin() && ! $query->get( 'flapjack_admin_search' ) ) {
            return false;
        }

        /**
         * Filter whether to intercept this search query.
         *
         * @param bool      $should_intercept
         * @param \WP_Query $query
         */
        return (bool) apply_filters( 'flapjack_should_intercept_query', true, $query );
    }

    /**
     * Fetch WP_Post objects in the given order.
     *
     * @param int[] $post_ids
     * @return \WP_Post[]
     */
    private function get_posts_in_order( array $post_ids ): array {
        if ( empty( $post_ids ) ) {
            return [];
        }

        // Use a single query with post__in and orderby=post__in.
        $fetched = get_posts( [
            'post__in'       => $post_ids,
            'orderby'        => 'post__in',
            'post_type'      => 'any',
            'post_status'    => 'publish',
            'posts_per_page' => count( $post_ids ),
            'flapjack_bypass' => true,
        ] );

        return $fetched;
    }

    /**
     * Check if backend search interception is enabled.
     */
    private function is_enabled(): bool {
        return (bool) get_option( 'flapjack_enable_search', true );
    }
}
