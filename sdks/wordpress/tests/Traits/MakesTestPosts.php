<?php
/**
 * Shared helper trait for creating test WP_Post objects.
 *
 * Eliminates the duplicated make_post() helper across test files.
 *
 * @package Flapjack\WordPress\Tests\Traits
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Tests\Traits;

trait MakesTestPosts {

    /**
     * Create a WP_Post with sensible defaults.
     *
     * @param array<string, mixed> $data Override any field.
     */
    protected function make_post( array $data = [] ): \WP_Post {
        $defaults = [
            'ID'                => 1,
            'post_author'       => '1',
            'post_date'         => '2026-01-15 10:00:00',
            'post_date_gmt'     => '2026-01-15 10:00:00',
            'post_content'      => 'Default test content.',
            'post_title'        => 'Test Post',
            'post_excerpt'      => '',
            'post_status'       => 'publish',
            'post_type'         => 'post',
            'post_password'     => '',
            'post_name'         => 'test-post',
            'post_modified'     => '2026-01-15 10:00:00',
            'post_modified_gmt' => '2026-01-15 10:00:00',
            'menu_order'        => 0,
            'comment_count'     => 0,
        ];

        return new \WP_Post( array_merge( $defaults, $data ) );
    }

    /**
     * Create a post and register it in the global store.
     *
     * @param array<string, mixed> $data Override any field.
     */
    protected function make_stored_post( array $data = [] ): \WP_Post {
        global $wp_posts_store;
        $post = $this->make_post( $data );
        $wp_posts_store[ $post->ID ] = $post;
        return $post;
    }
}
