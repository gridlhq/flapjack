<?php
/**
 * Manages the Flapjack search index — CRUD operations for WordPress content.
 *
 * @package Flapjack\WordPress\Indexing
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Indexing;

use Flapjack\WordPress\ClientFactory;

class IndexManager {

    private ClientFactory $client_factory;

    public function __construct( ClientFactory $client_factory ) {
        $this->client_factory = $client_factory;
    }

    /**
     * Index a single post.
     *
     * @param \WP_Post|int $post
     * @return array The API response.
     */
    public function index_post( \WP_Post|int $post ): array {
        if ( is_int( $post ) ) {
            $post = get_post( $post );
        }

        if ( ! $post instanceof \WP_Post ) {
            throw new \InvalidArgumentException( 'Invalid post.' );
        }

        if ( ! $this->should_index_post( $post ) ) {
            // If the post shouldn't be indexed, remove it in case it was previously indexed.
            return $this->delete_post( $post->ID );
        }

        $record = $this->build_record( $post );
        $client = $this->client_factory->get_client();
        $index  = $this->client_factory->get_index_name();

        return $client->saveObject( $index, $record );
    }

    /**
     * Delete a post from the index.
     *
     * @param int $post_id
     * @return array The API response.
     */
    public function delete_post( int $post_id ): array {
        $client = $this->client_factory->get_client();
        $index  = $this->client_factory->get_index_name();

        try {
            return $client->deleteObject( $index, (string) $post_id );
        } catch ( \Throwable $e ) {
            // Ignore 404 errors — the object may not exist in the index.
            if ( str_contains( $e->getMessage(), '404' ) || str_contains( $e->getMessage(), 'not found' ) ) {
                return [ 'deleted' => true ];
            }
            throw $e;
        }
    }

    /**
     * Reindex all content. Performs an atomic reindex using a temporary index.
     *
     * @return array{total: int, batches: int}
     */
    public function reindex_all(): array {
        $client     = $this->client_factory->get_client();
        $index_name = $this->client_factory->get_index_name();
        $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $total   = 0;
        $batches = 0;
        $batch   = [];
        $batch_size = 500;

        $query_args = [
            'post_type'      => $post_types,
            'post_status'    => 'publish',
            'posts_per_page' => $batch_size,
            'paged'          => 1,
            'orderby'        => 'ID',
            'order'          => 'ASC',
            // Disable Flapjack search interception for this query.
            'flapjack_bypass' => true,
        ];

        do {
            $query = new \WP_Query( $query_args );

            foreach ( $query->posts as $post ) {
                $batch[] = $this->build_record( $post );

                if ( count( $batch ) >= $batch_size ) {
                    $client->saveObjects( $index_name, $batch );
                    $total += count( $batch );
                    $batches++;
                    $batch = [];
                }
            }

            $query_args['paged']++;
        } while ( $query_args['paged'] <= $query->max_num_pages );

        // Flush remaining batch.
        if ( ! empty( $batch ) ) {
            $client->saveObjects( $index_name, $batch );
            $total += count( $batch );
            $batches++;
        }

        // Configure index settings.
        $this->configure_index_settings( $index_name );

        return [
            'total'   => $total,
            'batches' => $batches,
        ];
    }

    /**
     * Atomic reindex: zero-downtime index swap via temporary index + moveIndex.
     *
     * Indexes all content to a temporary index, configures settings on it,
     * then atomically moves it to replace the live index.
     *
     * @return array{total: int, batches: int, tmp_index: string}
     */
    public function reindex_atomic(): array {
        $client     = $this->client_factory->get_client();
        $index_name = $this->client_factory->get_index_name();
        $tmp_index  = $index_name . '_tmp_' . time();
        $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

        $total      = 0;
        $batches    = 0;
        $batch      = [];
        $batch_size = 500;

        $query_args = [
            'post_type'      => $post_types,
            'post_status'    => 'publish',
            'posts_per_page' => $batch_size,
            'paged'          => 1,
            'orderby'        => 'ID',
            'order'          => 'ASC',
            'flapjack_bypass' => true,
        ];

        do {
            $query = new \WP_Query( $query_args );

            foreach ( $query->posts as $post ) {
                $batch[] = $this->build_record( $post );

                if ( count( $batch ) >= $batch_size ) {
                    $client->saveObjects( $tmp_index, $batch );
                    $total += count( $batch );
                    $batches++;
                    $batch = [];
                }
            }

            $query_args['paged']++;
        } while ( $query_args['paged'] <= $query->max_num_pages );

        // Flush remaining batch.
        if ( ! empty( $batch ) ) {
            $client->saveObjects( $tmp_index, $batch );
            $total += count( $batch );
            $batches++;
        }

        // Configure index settings on the temporary index.
        $this->configure_index_settings( $tmp_index );

        // Atomic swap: move tmp → live (overwrites the live index).
        $client->operationIndex( $tmp_index, [
            'operation'   => 'move',
            'destination' => $index_name,
        ] );

        return [
            'total'     => $total,
            'batches'   => $batches,
            'tmp_index' => $tmp_index,
        ];
    }

    /**
     * Get index statistics.
     *
     * @return array{exists: bool, count: int, name: string}
     */
    public function get_index_stats(): array {
        $client     = $this->client_factory->get_client();
        $index_name = $this->client_factory->get_index_name();

        try {
            $settings = $client->getSettings( $index_name );
            // Try to get a count via an empty search.
            $result = $client->searchSingleIndex( $index_name, [
                'query'            => '',
                'hitsPerPage'      => 0,
                'analytics'        => false,
            ] );

            return [
                'exists' => true,
                'count'  => (int) ( $result['nbHits'] ?? 0 ),
                'name'   => $index_name,
            ];
        } catch ( \Throwable $e ) {
            return [
                'exists' => false,
                'count'  => 0,
                'name'   => $index_name,
            ];
        }
    }

    /**
     * Check whether a post should be indexed.
     */
    public function should_index_post( \WP_Post $post ): bool {
        $post_types = (array) get_option( 'flapjack_post_types', [ 'post', 'page' ] );

        if ( ! in_array( $post->post_type, $post_types, true ) ) {
            return false;
        }

        if ( 'publish' !== $post->post_status ) {
            return false;
        }

        if ( ! empty( $post->post_password ) ) {
            return false;
        }

        /**
         * Filter whether a specific post should be indexed.
         *
         * @param bool     $should_index Whether to index the post.
         * @param \WP_Post $post         The post object.
         */
        return (bool) apply_filters( 'flapjack_should_index_post', true, $post );
    }

    /**
     * Build a search record from a WP_Post.
     *
     * @param \WP_Post $post
     * @return array<string, mixed>
     */
    public function build_record( \WP_Post $post ): array {
        $record = [
            'objectID'       => (string) $post->ID,
            'post_id'        => $post->ID,
            'post_title'     => $post->post_title,
            'post_excerpt'   => $this->get_excerpt( $post ),
            'post_content'   => $this->get_clean_content( $post ),
            'post_type'      => $post->post_type,
            'post_type_label' => get_post_type_object( $post->post_type )?->labels->singular_name ?? $post->post_type,
            'post_status'    => $post->post_status,
            'post_date'      => strtotime( $post->post_date_gmt ) ?: 0,
            'post_modified'  => strtotime( $post->post_modified_gmt ) ?: 0,
            'permalink'      => get_permalink( $post ),
            'author'         => [
                'id'   => (int) $post->post_author,
                'name' => get_the_author_meta( 'display_name', (int) $post->post_author ),
            ],
        ];

        // Thumbnail.
        $thumbnail_id = get_post_thumbnail_id( $post );
        if ( $thumbnail_id ) {
            $record['thumbnail'] = wp_get_attachment_image_url( (int) $thumbnail_id, 'medium' );
        }

        // Taxonomies.
        $taxonomies = get_object_taxonomies( $post->post_type, 'objects' );
        foreach ( $taxonomies as $taxonomy ) {
            if ( ! $taxonomy->public ) {
                continue;
            }
            $terms = get_the_terms( $post, $taxonomy->name );
            if ( ! empty( $terms ) && ! is_wp_error( $terms ) ) {
                $record[ 'taxonomy_' . $taxonomy->name ] = array_map(
                    fn( \WP_Term $term ) => $term->name,
                    $terms
                );
            }
        }

        // Menu order (useful for pages).
        $record['menu_order'] = $post->menu_order;

        // Comment count.
        $record['comment_count'] = (int) $post->comment_count;

        /**
         * Filter the search record before indexing.
         *
         * @param array    $record The search record.
         * @param \WP_Post $post   The post object.
         */
        return (array) apply_filters( 'flapjack_post_record', $record, $post );
    }

    /**
     * Configure index settings (searchable attributes, facets, etc.).
     */
    public function configure_index_settings( string $index_name ): void {
        $client = $this->client_factory->get_client();

        $searchable = (array) get_option( 'flapjack_searchable_attrs', [ 'post_title', 'post_content', 'post_excerpt' ] );

        $searchable_attributes = [];
        if ( in_array( 'post_title', $searchable, true ) ) {
            $searchable_attributes[] = 'post_title';
        }
        if ( in_array( 'post_content', $searchable, true ) ) {
            $searchable_attributes[] = 'post_content';
        }
        if ( in_array( 'post_excerpt', $searchable, true ) ) {
            $searchable_attributes[] = 'post_excerpt';
        }
        if ( in_array( 'author', $searchable, true ) ) {
            $searchable_attributes[] = 'author.name';
        }

        $settings = [
            'searchableAttributes' => $searchable_attributes,
            'attributesForFaceting' => [
                'filterOnly(post_type)',
                'filterOnly(post_status)',
                'taxonomy_category',
                'taxonomy_post_tag',
                'author.name',
            ],
            'customRanking' => [
                'desc(post_date)',
            ],
            'attributesToSnippet' => [
                'post_content:30',
                'post_excerpt:30',
            ],
            'attributesToHighlight' => [
                'post_title',
                'post_content',
                'post_excerpt',
            ],
        ];

        /**
         * Filter the index settings before applying.
         *
         * @param array  $settings   The index settings.
         * @param string $index_name The index name.
         */
        $settings = (array) apply_filters( 'flapjack_index_settings', $settings, $index_name );

        $client->setSettings( $index_name, $settings );
    }

    /**
     * Get cleaned post content (strip shortcodes, blocks, HTML).
     */
    private function get_clean_content( \WP_Post $post ): string {
        $content = $post->post_content;
        $content = strip_shortcodes( $content );
        $content = excerpt_remove_blocks( $content );
        $content = wp_strip_all_tags( $content );
        $content = preg_replace( '/\s+/', ' ', $content ) ?? $content;

        // Truncate to ~10k chars to stay within index limits.
        if ( mb_strlen( $content ) > 10000 ) {
            $content = mb_substr( $content, 0, 10000 );
        }

        return trim( $content );
    }

    /**
     * Get the post excerpt, generating one if needed.
     */
    private function get_excerpt( \WP_Post $post ): string {
        if ( ! empty( $post->post_excerpt ) ) {
            return wp_strip_all_tags( $post->post_excerpt );
        }

        // Generate from content.
        $content = $this->get_clean_content( $post );
        return mb_strlen( $content ) > 300 ? mb_substr( $content, 0, 300 ) . '...' : $content;
    }
}
