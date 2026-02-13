<?php
/**
 * WP-CLI commands for Flapjack Search.
 *
 * Usage:
 *   wp flapjack reindex
 *   wp flapjack status
 *   wp flapjack index <post_id>
 *   wp flapjack delete <post_id>
 *   wp flapjack test
 *
 * @package Flapjack\WordPress\CLI
 */

declare(strict_types=1);

namespace Flapjack\WordPress\CLI;

use Flapjack\WordPress\ClientFactory;
use Flapjack\WordPress\Indexing\IndexManager;

class Commands {

    private ClientFactory $client_factory;
    private IndexManager $index_manager;

    public function __construct( ClientFactory $client_factory, IndexManager $index_manager ) {
        $this->client_factory = $client_factory;
        $this->index_manager  = $index_manager;
    }

    /**
     * Register WP-CLI commands.
     */
    public static function register( ClientFactory $client_factory, IndexManager $index_manager ): void {
        if ( ! defined( 'WP_CLI' ) || ! WP_CLI ) {
            return;
        }

        $instance = new self( $client_factory, $index_manager );

        \WP_CLI::add_command( 'flapjack reindex', [ $instance, 'reindex' ] );
        \WP_CLI::add_command( 'flapjack status', [ $instance, 'status' ] );
        \WP_CLI::add_command( 'flapjack index', [ $instance, 'index_post' ] );
        \WP_CLI::add_command( 'flapjack delete', [ $instance, 'delete_post' ] );
        \WP_CLI::add_command( 'flapjack test', [ $instance, 'test_connection' ] );
        \WP_CLI::add_command( 'flapjack search', [ $instance, 'search' ] );
    }

    /**
     * Reindex all content.
     *
     * ## OPTIONS
     *
     * [--batch-size=<size>]
     * : Number of posts per batch. Default 500.
     *
     * ## EXAMPLES
     *
     *     wp flapjack reindex
     *     wp flapjack reindex --batch-size=1000
     *
     * @param array $args
     * @param array $assoc_args
     */
    public function reindex( array $args, array $assoc_args ): void {
        if ( ! $this->client_factory->is_configured() ) {
            \WP_CLI::error( 'Flapjack Search is not configured. Set your API credentials first.' );
        }

        \WP_CLI::log( 'Starting full reindex...' );

        try {
            $result = $this->index_manager->reindex_all();
            \WP_CLI::success( sprintf(
                'Reindex complete. %d objects indexed in %d batches.',
                $result['total'],
                $result['batches']
            ) );
        } catch ( \Throwable $e ) {
            \WP_CLI::error( 'Reindex failed: ' . $e->getMessage() );
        }
    }

    /**
     * Show plugin and index status.
     *
     * ## EXAMPLES
     *
     *     wp flapjack status
     *
     * @param array $args
     * @param array $assoc_args
     */
    public function status( array $args, array $assoc_args ): void {
        $configured  = $this->client_factory->is_configured();
        $index_stats = $configured ? $this->index_manager->get_index_stats() : [ 'exists' => false, 'count' => 0, 'name' => '' ];
        $post_types  = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $wp_count = 0;
        foreach ( $post_types as $type ) {
            $counts = wp_count_posts( $type );
            $wp_count += (int) ( $counts->publish ?? 0 );
        }

        $rows = [
            [ 'Setting', 'Value' ],
            [ 'Plugin Version', FLAPJACK_SEARCH_VERSION ],
            [ 'Configured', $configured ? 'Yes' : 'No' ],
            [ 'App ID', $this->client_factory->get_app_id() ?: '(not set)' ],
            [ 'Host', $this->client_factory->get_host() ?: '(Flapjack Cloud)' ],
            [ 'Index Name', $this->client_factory->get_index_name() ],
            [ 'Index Exists', $index_stats['exists'] ? 'Yes' : 'No' ],
            [ 'Indexed Documents', (string) $index_stats['count'] ],
            [ 'WP Published Posts', (string) $wp_count ],
            [ 'Indexed Post Types', implode( ', ', $post_types ) ],
            [ 'Backend Search', get_option( 'flapjack_enable_search', true ) ? 'Enabled' : 'Disabled' ],
            [ 'Instant Search', get_option( 'flapjack_enable_instant', false ) ? 'Enabled' : 'Disabled' ],
        ];

        $table = new \cli\Table();
        $table->setHeaders( $rows[0] );
        $table->setRows( array_slice( $rows, 1 ) );
        $table->display();
    }

    /**
     * Index a specific post.
     *
     * ## OPTIONS
     *
     * <post_id>
     * : The post ID to index.
     *
     * ## EXAMPLES
     *
     *     wp flapjack index 42
     *
     * @param array $args
     * @param array $assoc_args
     */
    public function index_post( array $args, array $assoc_args ): void {
        $post_id = (int) $args[0];
        $post    = get_post( $post_id );

        if ( ! $post ) {
            \WP_CLI::error( sprintf( 'Post %d not found.', $post_id ) );
        }

        try {
            $this->index_manager->index_post( $post );
            \WP_CLI::success( sprintf( 'Post %d indexed successfully.', $post_id ) );
        } catch ( \Throwable $e ) {
            \WP_CLI::error( 'Indexing failed: ' . $e->getMessage() );
        }
    }

    /**
     * Delete a post from the index.
     *
     * ## OPTIONS
     *
     * <post_id>
     * : The post ID to delete from the index.
     *
     * ## EXAMPLES
     *
     *     wp flapjack delete 42
     *
     * @param array $args
     * @param array $assoc_args
     */
    public function delete_post( array $args, array $assoc_args ): void {
        $post_id = (int) $args[0];

        try {
            $this->index_manager->delete_post( $post_id );
            \WP_CLI::success( sprintf( 'Post %d removed from index.', $post_id ) );
        } catch ( \Throwable $e ) {
            \WP_CLI::error( 'Deletion failed: ' . $e->getMessage() );
        }
    }

    /**
     * Test the Flapjack connection.
     *
     * ## EXAMPLES
     *
     *     wp flapjack test
     *
     * @param array $args
     * @param array $assoc_args
     */
    public function test_connection( array $args, array $assoc_args ): void {
        $result = $this->client_factory->test_connection();

        if ( $result['success'] ) {
            \WP_CLI::success( $result['message'] );
        } else {
            \WP_CLI::error( $result['message'] );
        }
    }

    /**
     * Search the index from the command line.
     *
     * ## OPTIONS
     *
     * <query>
     * : The search query.
     *
     * [--per-page=<n>]
     * : Number of results. Default 10.
     *
     * ## EXAMPLES
     *
     *     wp flapjack search "hello world"
     *     wp flapjack search "test" --per-page=5
     *
     * @param array $args
     * @param array $assoc_args
     */
    public function search( array $args, array $assoc_args ): void {
        $query    = $args[0];
        $per_page = (int) ( $assoc_args['per-page'] ?? 10 );

        try {
            $client = $this->client_factory->get_client();
            $index  = $this->client_factory->get_index_name();
            $result = $client->searchSingleIndex( $index, [
                'query'       => $query,
                'hitsPerPage' => $per_page,
            ] );

            $hits = $result['hits'] ?? [];
            if ( empty( $hits ) ) {
                \WP_CLI::log( 'No results found.' );
                return;
            }

            \WP_CLI::log( sprintf( 'Found %d results (showing %d):', $result['nbHits'] ?? count( $hits ), count( $hits ) ) );

            foreach ( $hits as $hit ) {
                \WP_CLI::log( sprintf(
                    '  [%s] %s — %s',
                    $hit['objectID'] ?? '?',
                    $hit['post_title'] ?? '(no title)',
                    $hit['permalink'] ?? ''
                ) );
            }
        } catch ( \Throwable $e ) {
            \WP_CLI::error( 'Search failed: ' . $e->getMessage() );
        }
    }
}
