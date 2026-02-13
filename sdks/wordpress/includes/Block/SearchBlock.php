<?php
/**
 * Gutenberg block registration and server-side rendering for Flapjack Search.
 *
 * @package Flapjack\WordPress\Block
 */

declare(strict_types=1);

namespace Flapjack\WordPress\Block;

class SearchBlock {

    /**
     * Register hooks for the search block.
     */
    public function register(): void {
        add_action( 'init', [ $this, 'register_block' ] );
    }

    /**
     * Register the block type from block.json metadata.
     */
    public function register_block(): void {
        $block_dir = FLAPJACK_SEARCH_DIR . 'blocks/flapjack-search';

        if ( ! file_exists( $block_dir . '/block.json' ) ) {
            return;
        }

        register_block_type( $block_dir );

        // Pass search config to the view script via inline script.
        $this->localize_block_config();
    }

    /**
     * Pass Flapjack config to the block's view script so autocomplete can work.
     */
    private function localize_block_config(): void {
        $app_id     = (string) get_option( 'flapjack_app_id', '' );
        $search_key = (string) get_option( 'flapjack_search_api_key', '' );
        $host       = (string) get_option( 'flapjack_host', '' );
        $index_name = (string) get_option( 'flapjack_index_name', 'wp_posts' );

        // Never expose the admin API key in page source.
        // If no search-only key is configured, skip block autocomplete.
        if ( empty( $search_key ) ) {
            return;
        }

        $config = [
            'appId'     => $app_id,
            'apiKey'    => $search_key,
            'indexName' => $index_name,
            'host'      => $host,
            'restUrl'   => esc_url_raw( rest_url( 'flapjack-search/v1/' ) ),
            'nonce'     => wp_create_nonce( 'wp_rest' ),
        ];

        wp_add_inline_script(
            'flapjack-search-view-script',
            'var flapjackSearchConfig = ' . wp_json_encode( $config ) . ';',
            'before'
        );
    }

    /**
     * Get the path to the blocks directory.
     */
    public static function get_block_dir(): string {
        return FLAPJACK_SEARCH_DIR . 'blocks/flapjack-search';
    }
}
